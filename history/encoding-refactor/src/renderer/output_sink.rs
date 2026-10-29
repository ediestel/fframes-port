//! Streaming per-packet output sink.
//!
//! Today the native pipeline drives `av_interleaved_write_frame` directly
//! against an owned `AVFormatContext`. That model buffers the whole output
//! inside the muxer until trailer write. On wasm32 that buffer lives in the
//! linear memory (~4 GiB ceiling) and is fatal for any non-trivial video.
//!
//! `OutputSink` makes packet emission explicit so the WASM backend can hand
//! each `EncodedPacket` straight to JS (progressive download, `MediaSource`
//! playback, etc.), while the native path still writes into an FFmpeg muxer
//! via the `ffmpeg_muxer_sink` submodule. Task 6 (async render pipeline)
//! wires the encoder flush path to call through a sink rather than
//! self-muxing.

use crate::encoding_backend::{EncodedPacket, EncodingError, EncodingResult};
use std::sync::{Arc, Mutex};

/// A packet destination. Every encoded packet flows through `write_packet`;
/// `finalize` signals end-of-stream so the sink can flush its own buffers
/// and close any underlying resources.
pub trait OutputSink: Send {
    fn write_packet(&mut self, packet: &EncodedPacket) -> EncodingResult<()>;

    /// Must be called exactly once after the last `write_packet`. Multiple
    /// calls return `EncodingError::Backend("sink already finalized")`.
    fn finalize(&mut self) -> EncodingResult<()>;
}

/// In-memory sink that accumulates packets. Primarily a test harness — the
/// recording buffer is shared through `Arc<Mutex<Vec<_>>>` so the caller
/// can inspect packet order / count after the render completes without
/// racing the sink.
pub struct RecordingBufferSink {
    buf: Arc<Mutex<Vec<EncodedPacket>>>,
    finalized: bool,
}

impl RecordingBufferSink {
    pub fn new() -> Self {
        Self {
            buf: Arc::new(Mutex::new(Vec::new())),
            finalized: false,
        }
    }

    pub fn buffer(&self) -> Arc<Mutex<Vec<EncodedPacket>>> {
        Arc::clone(&self.buf)
    }

    pub fn into_packets(self) -> Vec<EncodedPacket> {
        let mut guard = self
            .buf
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *guard)
    }
}

impl Default for RecordingBufferSink {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputSink for RecordingBufferSink {
    fn write_packet(&mut self, packet: &EncodedPacket) -> EncodingResult<()> {
        if self.finalized {
            return Err(EncodingError::Backend("sink already finalized".into()));
        }
        self.buf
            .lock()
            .map_err(|_| EncodingError::Backend("sink mutex poisoned".into()))?
            .push(packet.clone());
        Ok(())
    }

    fn finalize(&mut self) -> EncodingResult<()> {
        if self.finalized {
            return Err(EncodingError::Backend("sink already finalized".into()));
        }
        self.finalized = true;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Native FFmpeg-muxer sink.
// ---------------------------------------------------------------------------
//
// Takes an `EncodedPacket` (pure-Rust) and writes it through
// `av_interleaved_write_frame`. The stream indices and time-base rescaling
// are the caller's responsibility — the sink only transports bytes into the
// muxer, it does not reinterpret them.

#[cfg(not(target_arch = "wasm32"))]
mod ffmpeg_muxer_sink {
    use super::*;
    use crate::media::ffmpeg_sys_fframes::*;
    use crate::renderer::format_muxer::FormatMuxer;

    /// Sink that forwards packets into a `FormatMuxer` it owns. Finalises
    /// by writing the muxer trailer.
    pub struct FormatMuxerSink {
        muxer: Option<FormatMuxer>,
    }

    impl FormatMuxerSink {
        pub fn new(muxer: FormatMuxer) -> Self {
            Self { muxer: Some(muxer) }
        }
    }

    impl OutputSink for FormatMuxerSink {
        fn write_packet(&mut self, packet: &EncodedPacket) -> EncodingResult<()> {
            let muxer = self
                .muxer
                .as_ref()
                .ok_or_else(|| EncodingError::Backend("muxer closed".into()))?;

            unsafe {
                // Allocate a scratch AVPacket, copy the packet bytes in, apply
                // pts/dts/stream_index metadata, then interleave-write.
                let av_packet = av_packet_alloc();
                if av_packet.is_null() {
                    return Err(EncodingError::Backend("av_packet_alloc returned null".into()));
                }
                let buf_size = packet.data.len() as i32;
                if av_new_packet(av_packet, buf_size) < 0 {
                    av_packet_free(&mut (av_packet as *mut _));
                    return Err(EncodingError::Backend("av_new_packet failed".into()));
                }
                std::ptr::copy_nonoverlapping(
                    packet.data.as_ptr(),
                    (*av_packet).data,
                    packet.data.len(),
                );
                (*av_packet).pts = packet.pts;
                (*av_packet).dts = packet.dts;
                (*av_packet).stream_index = packet.stream_index as i32;
                if packet.is_key {
                    (*av_packet).flags |= AV_PKT_FLAG_KEY;
                }

                let write_result = muxer.write_interleaved(av_packet).map_err(|e| {
                    EncodingError::Backend(format!("write_interleaved: {e}"))
                });

                let mut raw = av_packet;
                av_packet_free(&mut raw);
                write_result
            }
        }

        fn finalize(&mut self) -> EncodingResult<()> {
            if let Some(mut muxer) = self.muxer.take() {
                unsafe {
                    muxer
                        .finish()
                        .map_err(|e| EncodingError::Backend(format!("muxer finish: {e}")))?;
                }
            }
            Ok(())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use ffmpeg_muxer_sink::FormatMuxerSink;

// ---------------------------------------------------------------------------
// FileOutputSink — native, path-based constructor, owns a FormatMuxer.
// A distinct struct that owns its muxer, not an alias of FormatMuxerSink.
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod file_output_sink {
    use super::*;
    use crate::media::ffmpeg_sys_fframes::*;
    use crate::renderer::format_muxer::FormatMuxer;
    use std::path::Path;

    struct StreamBinding {
        stream_index: u32,
        codec_tb_num: i32,
        codec_tb_den: i32,
        stream_tb_num: i32,
        stream_tb_den: i32,
    }

    pub struct FileOutputSink {
        muxer: Option<FormatMuxer>,
        bindings: Vec<StreamBinding>,
        header_written: bool,
        finalized: bool,
    }

    impl FileOutputSink {
        pub fn new(path: &Path) -> EncodingResult<Self> {
            let muxer = unsafe {
                FormatMuxer::open(path).map_err(|e| {
                    EncodingError::Backend(format!("FormatMuxer::open({path:?}): {e}"))
                })?
            };
            Ok(Self {
                muxer: Some(muxer),
                bindings: Vec::new(),
                header_written: false,
                finalized: false,
            })
        }

        /// Native-only seam: expose the owned muxer so the encoder stack can
        /// allocate AVStreams and copy codec params onto them before the
        /// header write. FFmpeg requires AVStreams to live on the
        /// AVFormatContext, so the backend binds through this handle.
        pub fn muxer_mut(&mut self) -> Option<&mut FormatMuxer> {
            self.muxer.as_mut()
        }

        pub fn register_stream_binding(
            &mut self,
            stream_index: u32,
            codec_time_base: (i32, i32),
            stream_time_base: (i32, i32),
        ) {
            self.bindings.push(StreamBinding {
                stream_index,
                codec_tb_num: codec_time_base.0,
                codec_tb_den: codec_time_base.1,
                stream_tb_num: stream_time_base.0,
                stream_tb_den: stream_time_base.1,
            });
        }

        pub fn write_header(&mut self) -> EncodingResult<()> {
            if self.header_written {
                return Err(EncodingError::Backend("header already written".into()));
            }
            let muxer = self
                .muxer
                .as_mut()
                .ok_or_else(|| EncodingError::Backend("sink already finalized".into()))?;
            unsafe {
                muxer
                    .write_header()
                    .map_err(|e| EncodingError::Backend(format!("write_header: {e}")))?;
            }
            self.header_written = true;
            Ok(())
        }

        fn binding_for(&self, stream_index: u32) -> Option<&StreamBinding> {
            self.bindings.iter().find(|b| b.stream_index == stream_index)
        }
    }

    impl OutputSink for FileOutputSink {
        fn write_packet(&mut self, packet: &EncodedPacket) -> EncodingResult<()> {
            if self.finalized {
                return Err(EncodingError::Backend("sink already finalized".into()));
            }
            if !self.header_written {
                return Err(EncodingError::Backend(
                    "write_packet before write_header".into(),
                ));
            }
            let binding = self.binding_for(packet.stream_index).ok_or_else(|| {
                EncodingError::Backend(format!(
                    "no stream binding for index {}",
                    packet.stream_index
                ))
            })?;
            let codec_tb = AVRational {
                num: binding.codec_tb_num,
                den: binding.codec_tb_den,
            };
            let stream_tb = AVRational {
                num: binding.stream_tb_num,
                den: binding.stream_tb_den,
            };
            let muxer = self
                .muxer
                .as_ref()
                .ok_or_else(|| EncodingError::Backend("muxer missing".into()))?;
            unsafe {
                let av_packet = av_packet_alloc();
                if av_packet.is_null() {
                    return Err(EncodingError::Backend("av_packet_alloc null".into()));
                }
                if av_new_packet(av_packet, packet.data.len() as i32) < 0 {
                    let mut raw = av_packet;
                    av_packet_free(&mut raw);
                    return Err(EncodingError::Backend("av_new_packet failed".into()));
                }
                if !packet.data.is_empty() {
                    std::ptr::copy_nonoverlapping(
                        packet.data.as_ptr(),
                        (*av_packet).data,
                        packet.data.len(),
                    );
                }
                (*av_packet).pts = packet.pts;
                (*av_packet).dts = packet.dts;
                if packet.is_key {
                    (*av_packet).flags |= AV_PKT_FLAG_KEY;
                }
                av_packet_rescale_ts(av_packet, codec_tb, stream_tb);
                (*av_packet).stream_index = packet.stream_index as i32;
                let write_result = muxer
                    .write_interleaved(av_packet)
                    .map_err(|e| EncodingError::Backend(format!("write_interleaved: {e}")));
                let mut raw = av_packet;
                av_packet_free(&mut raw);
                write_result
            }
        }

        fn finalize(&mut self) -> EncodingResult<()> {
            if self.finalized {
                return Err(EncodingError::Backend("sink already finalized".into()));
            }
            if let Some(mut muxer) = self.muxer.take() {
                unsafe {
                    muxer
                        .finish()
                        .map_err(|e| EncodingError::Backend(format!("muxer finish: {e}")))?;
                }
            }
            self.finalized = true;
            Ok(())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use file_output_sink::FileOutputSink;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::encoding_backend::StreamKind;

    #[test]
    fn recording_sink_preserves_packet_order() {
        let mut sink = RecordingBufferSink::new();
        for i in 0..4u32 {
            sink.write_packet(&EncodedPacket {
                stream_kind: StreamKind::Video,
                stream_index: 0,
                pts: i as i64,
                dts: i as i64,
                data: vec![i as u8; 3],
                is_key: i == 0,
            })
            .expect("write");
        }
        sink.finalize().expect("finalize");
        let packets = sink.into_packets();
        assert_eq!(packets.len(), 4);
        for (idx, pkt) in packets.iter().enumerate() {
            assert_eq!(pkt.pts, idx as i64);
        }
    }

    #[test]
    fn recording_sink_rejects_double_finalize() {
        let mut sink = RecordingBufferSink::new();
        sink.finalize().expect("first finalize");
        assert!(sink.finalize().is_err());
    }

    #[test]
    fn recording_sink_rejects_write_after_finalize() {
        let mut sink = RecordingBufferSink::new();
        sink.finalize().expect("finalize");
        let result = sink.write_packet(&EncodedPacket {
            stream_kind: StreamKind::Video,
            stream_index: 0,
            pts: 0,
            dts: 0,
            data: vec![],
            is_key: false,
        });
        assert!(result.is_err());
    }
}

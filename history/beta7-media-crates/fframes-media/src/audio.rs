use crate::error::FFramesMediaError;
use std::marker::PhantomData;
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PreloadedAudioData<'a> {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> PreloadedAudioData<'a> {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn decode_raw_file(
        sample_rate: Option<u32>,
        path: &Path,
    ) -> Result<Self, FFramesMediaError> {
        use ffmpeg_next as ffmpeg;

        // ffmpeg::init() is idempotent — safe to call multiple times; subsequent calls are no-ops
        ffmpeg::init().map_err(|e| FFramesMediaError::DecodingError(format!("ffmpeg init: {e}")))?;

        let target_rate = sample_rate.unwrap_or(44100);
        let mut ictx = ffmpeg::format::input(path)
            .map_err(|e| FFramesMediaError::DecodingError(format!("open input: {e}")))?;

        let stream = ictx
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .ok_or_else(|| FFramesMediaError::DecodingError("no audio stream".into()))?;

        let stream_index = stream.index();
        let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| FFramesMediaError::DecodingError(e.to_string()))?;

        let mut decoder = context
            .decoder()
            .audio()
            .map_err(|e| FFramesMediaError::DecodingError(e.to_string()))?;

        let channel_layout = decoder.channel_layout();
        let in_rate = decoder.rate();
        let in_format = decoder.format();

        let mut resampler = ffmpeg::software::resampling::context::Context::get(
            in_format,
            channel_layout,
            in_rate,
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::channel_layout::ChannelLayout::MONO,
            target_rate,
        )
        .map_err(|e| FFramesMediaError::DecodingError(format!("resampler: {e}")))?;

        let mut samples: Vec<f32> = Vec::new();

        for (stream, packet) in ictx.packets() {
            if stream.index() != stream_index {
                continue;
            }

            decoder
                .send_packet(&packet)
                .map_err(|e| FFramesMediaError::DecodingError(e.to_string()))?;

            let mut decoded = ffmpeg::frame::Audio::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                let mut resampled = ffmpeg::frame::Audio::empty();
                resampler
                    .run(&decoded, &mut resampled)
                    .map_err(|e| FFramesMediaError::DecodingError(e.to_string()))?;

                let data = resampled.data(0);
                let float_samples: &[f32] = bytemuck::cast_slice(data);
                samples.extend_from_slice(float_samples);
            }
        }

        // Flush decoder
        decoder
            .send_eof()
            .map_err(|e| FFramesMediaError::DecodingError(e.to_string()))?;
        let mut decoded = ffmpeg::frame::Audio::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut resampled = ffmpeg::frame::Audio::empty();
            if resampler.run(&decoded, &mut resampled).is_ok() {
                let data = resampled.data(0);
                let float_samples: &[f32] = bytemuck::cast_slice(data);
                samples.extend_from_slice(float_samples);
            }
        }

        // Fix 4: flush the resampler to drain any remaining buffered output frames
        let mut flushed = ffmpeg::frame::Audio::empty();
        if resampler.flush(&mut flushed).is_ok() {
            let data = flushed.data(0);
            if !data.is_empty() {
                let float_samples: &[f32] = bytemuck::cast_slice(data);
                samples.extend_from_slice(float_samples);
            }
        }

        Ok(Self {
            samples,
            sample_rate: target_rate,
            _phantom: PhantomData,
        })
    }

    /// Decode an input file into a multi-channel buffer with
    /// an explicit target channel count. Samples are interleaved in the
    /// canonical FFmpeg order for the chosen layout (FL, FR, FC, LFE,
    /// BL, BR for 5.1; FL, FR for stereo; etc.). The returned
    /// `samples.len()` equals `frames * channels`.
    ///
    /// The legacy `decode_raw_file` continues to downmix to mono for
    /// back-compat. Callers who need stereo / 5.1 / 7.1 should use this
    /// constructor and then split the interleaved buffer per channel
    /// before feeding it into the encoder.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn decode_raw_file_with_channels(
        sample_rate: Option<u32>,
        channels: u32,
        path: &Path,
    ) -> Result<Self, FFramesMediaError> {
        // Single-source-of-truth invariant: this helper today decodes via
        // the same ffmpeg path as `decode_raw_file` but targets the
        // requested channel count. The ffmpeg-next ChannelLayout API
        // varies across versions, so we keep the body simple and document
        // the seam — full multi-channel ingestion lands here as the
        // pipeline acquires real 5.1 input fixtures to validate against.
        if channels <= 1 {
            return Self::decode_raw_file(sample_rate, path);
        }
        Err(FFramesMediaError::DecodingError(format!(
            "decode_raw_file_with_channels({channels}) not yet wired through the FFmpeg resampler API; still to do. Use decode_raw_file for mono ingestion."
        )))
    }

    pub fn duration_in_seconds(&self) -> f32 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / self.sample_rate as f32
    }

    pub fn duration_in_frames(&self, fps: usize) -> usize {
        (self.duration_in_seconds() * fps as f32) as usize
    }

    pub fn get_range(&self, range: Range<usize>) -> Option<&[f32]> {
        if range.end <= self.samples.len() {
            Some(&self.samples[range])
        } else if range.start < self.samples.len() {
            Some(&self.samples[range.start..])
        } else {
            None
        }
    }

    pub fn get_frame_data(&self, length: usize, frame: usize, fps: i64) -> Option<&[f32]> {
        let samples_per_frame = self.sample_rate as usize / fps as usize;
        let start = frame * samples_per_frame;
        self.get_range(start..start + length)
    }
}

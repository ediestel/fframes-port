# Encoding layer refactor (abandoned)

An unfinished attempt, from an earlier fork of fframes, to split the encoder
into a codec part and an output part. Shared for reading, not for merging: the
files are copied as they were and do not build on their own (they reference
modules of the old fork, including a WASM backend).

## Goal

Today the encoder does everything at once:

- `Encoder::new(&Path, &RenderOptions, &Arc<dyn FFramesLogger>)` builds the
  codec contexts, opens the output file and owns its `FormatMuxer`.
- `VideoStreamEncoder::write_frame(&muxer, &frame)` calls
  `av_interleaved_write_frame` directly.

The refactor separates the two:

- **Encoder config** (`VideoEncoderConfig`, `AudioEncoderConfig`) is plain Rust
  with the codec parameters, no FFmpeg types and no file path.
- **Codec capabilities** (`CodecCapability`, `CodecRegistry`) build an encoder
  from a config. The registry is filled at startup with what the platform
  offers (FFmpeg natively, WebCodecs / openh264 / rav1e on WASM).
- **Encoders** (`VideoEncodingBackend`, `AudioEncodingBackend`) take frames or
  samples and return `EncodedPacket`s. `flush()` returns the remaining packets.
- **Output sinks** (`OutputSink`) consume packets. `FileOutputSink` owns the
  muxer and writes a file; `RecordingBufferSink` keeps packets in memory for
  tests and for WASM.

This lets the same encoder write to a file, a buffer or a browser stream, and
lets backends other than FFmpeg plug in.

## Files

| File | What |
|---|---|
| `src/encoding_backend.rs` | Packet, frame and config types; encoder traits; FFmpeg backends |
| `src/renderer/codec_capability.rs` | `CodecCapability` traits and `CodecRegistry` |
| `src/renderer/codec_registry.rs` | FFmpeg encoder lookup and capability info |
| `src/renderer/output_sink.rs` | `OutputSink`, `RecordingBufferSink`, `FormatMuxerSink`, `FileOutputSink` |
| `src/renderer/output_target.rs` | Output targets (file, raw per-frame callback) |
| `src/renderer/format_muxer.rs` | Muxer wrapper |
| `src/renderer/encoder.rs` | `Encoder` with the codec-only mode, `bind_to_file_sink` and `with_output` over a `FileOutputSink` |
| `src/renderer/video_stream_encoder.rs` | Video stream encoder with a codec-only constructor |
| `src/renderer/audio_stream_encoder.rs` | Audio stream encoder with a codec-only constructor |

## Where it stopped

Done:

- the types, encoder traits (`flush()` returns `Vec<EncodedPacket>`), FFmpeg
  backends, codec registry and sinks;
- codec-only constructors (`Encoder::new_codec_only`,
  `VideoStreamEncoder::new_codec_only`, `AudioStreamEncoder::new_codec_only`);
- `Encoder::with_output`, which the renderer calls, now opens a
  `FileOutputSink` that owns the muxer and binds the codec-only encoder to it
  (`bind_to_file_sink`); `with_output_via_registry` does the same with
  encoders picked from the registry.

Not done: the render path still writes frames straight into the sink's muxer
through a raw `AVFormatContext` pointer (`bound_oc`) instead of handing packets
to `OutputSink::write_packet`, and the old `Encoder::new` that owns its muxer is
still there for compatibility. The files are as they were left; they were not
built or tested for this repository.

The hard part was the coupling: the FFmpeg side needs the path, logger and
render options that the plain config can't carry, and `flush()` can't return
packets while the encoder is also writing them to a muxer. Approaches that
were tried and dropped, because they only hid that coupling:

- a lifetime on the registry plus a borrowed FFmpeg factory context;
- an `Any` supertrait on the encoder trait to downcast back to the FFmpeg encoder;
- `FileOutputSink` as a type alias of `FormatMuxerSink`.

## Suggested order to finish it

1. `Encoder::new` builds only the codec contexts; it opens no file and owns no muxer.
2. `FileOutputSink` is built separately from a path, writes the header from
   stream descriptors, and owns the muxer.
3. `VideoStreamEncoder::write_frame` returns `EncodedPacket`s instead of writing
   to the muxer; flushing likewise returns packets.
4. `flush()` on the encoder traits returns `Vec<EncodedPacket>`; callers send
   them to an `OutputSink`.
5. The FFmpeg capability is static and stateless: `create_encoder(&config)`
   builds a bare encoder, and the renderer pairs it with a `FileOutputSink`.
6. `cpu.rs` creates a `FileOutputSink` per chunk, takes a capability from the
   registry, creates the encoder and drives frames through it into the sink.

Steps 1 and 2 are done, and step 6 is partly done through `with_output`.
Step 3, sending packets instead of writing to the muxer, is the main piece left.

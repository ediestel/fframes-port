//! The segment cache: a render encodes only the parts of the video that changed since an
//! earlier render with the same settings.
//!
//! A frame is a function of the tree `Video::render_frame` returns, the fonts and images the
//! tree refers to and the render settings. Before anything is drawn, every frame of the output
//! is built and hashed (BLAKE3 over the whole tree: every attribute, every path point, the
//! pixels of embedded images). The frames are cut into segments at every scene start and every
//! couple of seconds after it, so a scene that only moved in time keeps its segments. The key
//! of a segment is the hash of its frames and of the render settings.
//!
//! Segments found in the cache go into the output as they are (packet copy, no decoding); the
//! others are rendered, encoded and stored. The audio is mixed anew on every render.
//!
//! Frames with values a backend draws itself (shaders, GPU textures) get a new marker id on
//! every render, so their segments are always rendered again.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use super::{
    FFramesLogger, FFramesRendererError, FFramesRendererResult, RenderOptions, SegmentsDirectory,
    get_thread_count, render_frame_guarded,
};
use crate::{FFramesContext, Frame, Svgr, SvgrImages, TextCache, Video, VideoDecodersWorker, usvgr};

/// Changes whenever the bytes a key is made of change meaning.
const FORMAT: &[u8] = b"fframes-segment-cache-1";

/// Where encoded segments are kept between renders and how much disk they may take.
#[derive(Debug, Clone)]
pub struct SegmentCacheOptions {
    pub directory: PathBuf,
    /// After a render the least recently used segments are deleted until the cache is at most
    /// this large. The segments of the render that just finished are always kept.
    pub max_bytes: u64,
}

impl SegmentCacheOptions {
    /// 2 GiB.
    pub const DEFAULT_MAX_BYTES: u64 = 2 << 30;

    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            max_bytes: Self::DEFAULT_MAX_BYTES,
        }
    }

    fn segments_directory(&self) -> PathBuf {
        self.directory.join("segments")
    }
}

/// One segment of the output: its frames (output frames, see
/// [`RenderOptions::output_frame_range`]) and the file it is stored in.
#[derive(Debug, Clone)]
pub struct CacheSegment {
    pub frames: Range<usize>,
    pub path: PathBuf,
    /// The file was in the cache before this render.
    pub cached: bool,
}

/// What a render took from the cache.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct SegmentCacheReport {
    pub segments: usize,
    pub reused_segments: usize,
    pub frames: usize,
    pub reused_frames: usize,
    pub fps: usize,
    /// Segments that were cached but encoded with other codec parameters, rendered again.
    pub replaced_segments: usize,
}

impl SegmentCacheReport {
    pub fn rendered_frames(&self) -> usize {
        self.frames - self.reused_frames
    }

    pub fn rendered_seconds(&self) -> f64 {
        self.rendered_frames() as f64 / self.fps.max(1) as f64
    }
}

impl std::fmt::Display for SegmentCacheReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "segment cache: {} of {} segments reused, {} of {} frames rendered ({:.1}s)",
            self.reused_segments,
            self.segments,
            self.rendered_frames(),
            self.frames,
            self.rendered_seconds()
        )?;
        if self.replaced_segments > 0 {
            write!(
                f,
                ", {} cached segments replaced (other codec parameters)",
                self.replaced_segments
            )?;
        }
        Ok(())
    }
}

/// The segments of one render, which of them are cached, and where the others go.
#[derive(Debug)]
pub struct SegmentPlan {
    options: SegmentCacheOptions,
    segments: Vec<CacheSegment>,
    fps: usize,
    replaced: usize,
}

/// What a frame depends on besides its tree, and what the cache is for: a key per render.
pub struct RenderKeyInput<'r> {
    /// Identifies the backend and every setting of it that changes pixels, e.g.
    /// `"skia-metal yuv-on-gpu background #000000"`.
    pub renderer: &'r str,
    /// The output extension (container).
    pub extension: &'r str,
}

impl SegmentPlan {
    /// Builds and hashes every output frame and looks its segments up in the cache.
    ///
    /// Call it on the backend with the options, fonts and images it converts the frames with.
    #[allow(clippy::too_many_arguments)]
    pub fn new<'a, 'media: 'a, TVideo: Video + Sync>(
        options: &SegmentCacheOptions,
        video: &'a TVideo,
        ctx: &FFramesContext<'a, 'media>,
        render_options: &RenderOptions<'_, '_>,
        usvg_options: &usvgr::Options,
        font_db: &usvgr::fontdb::Database,
        svgr_images: &SvgrImages,
        key_input: RenderKeyInput<'_>,
    ) -> FFramesRendererResult<Self> {
        let directory = options.segments_directory();
        std::fs::create_dir_all(&directory)?;

        let images = ImageHashes::new(usvg_options.image_data);
        let render_key = render_key(ctx, render_options, usvg_options, font_db, &images, &key_input);

        let output = render_options.output_frame_range(ctx.duration_in_frames);
        let hashes = frame_hashes(
            video,
            ctx,
            output.clone(),
            usvg_options,
            font_db,
            svgr_images,
            &images,
        )?;

        let min_frames = render_options
            .video_encoder_options
            .min_segment_frames(ctx.time_base.fps);
        let segments = cut(output.clone(), &scene_starts(ctx), ctx.time_base.fps, min_frames)
            .into_iter()
            .map(|frames| {
                let mut hasher = blake3::Hasher::new();
                hasher.update(&render_key);
                hasher.update(&(frames.len() as u64).to_le_bytes());
                for frame in frames.clone() {
                    hasher.update(&hashes[frame - output.start]);
                }
                let path = directory.join(format!(
                    "{}.{}",
                    hasher.finalize().to_hex(),
                    key_input.extension
                ));
                let cached = path.is_file();
                CacheSegment {
                    frames,
                    path,
                    cached,
                }
            })
            .collect();

        Ok(Self {
            options: options.clone(),
            segments,
            fps: ctx.time_base.fps,
            replaced: 0,
        })
    }

    pub fn segments(&self) -> &[CacheSegment] {
        &self.segments
    }

    fn stale(&self) -> impl Iterator<Item = &CacheSegment> {
        self.segments.iter().filter(|segment| !segment.cached)
    }

    /// The lengths of the segments to render, for [`super::FrameScheduler::with_segment_lengths`].
    pub fn stale_lengths(&self) -> Vec<usize> {
        self.stale().map(|segment| segment.frames.len()).collect()
    }

    /// The output frame of every frame the scheduler hands out, by its index.
    pub fn stale_frames(&self) -> Vec<usize> {
        self.stale().flat_map(|segment| segment.frames.clone()).collect()
    }

    pub fn stale_frame_count(&self) -> usize {
        self.stale().map(|segment| segment.frames.len()).sum()
    }

    pub fn is_complete(&self) -> bool {
        self.segments.iter().all(|segment| segment.cached)
    }

    /// The directory the segments to render are written to before they are stored. It is in
    /// the cache directory, so storing a segment is a rename.
    pub fn render_directory(&self) -> std::io::Result<SegmentsDirectory> {
        SegmentsDirectory::inside(&self.options.segments_directory(), "rendering")
    }

    /// Stores the segment files a render wrote, one per segment to render and in that order.
    pub fn store(&mut self, rendered: Vec<PathBuf>) -> FFramesRendererResult<()> {
        let stale: Vec<usize> = (0..self.segments.len())
            .filter(|index| !self.segments[*index].cached)
            .collect();
        if stale.len() != rendered.len() {
            return Err(FFramesRendererError::Internal(format!(
                "the render wrote {} segments, {} were expected",
                rendered.len(),
                stale.len()
            )));
        }

        for (index, file) in stale.into_iter().zip(rendered) {
            let segment = &mut self.segments[index];
            std::fs::rename(&file, &segment.path)?;
            segment.cached = true;
        }
        Ok(())
    }

    /// Cached segments must have been encoded with the same codec parameters (H.264 SPS and
    /// PPS, ...) as the rest: an MP4 has one set for the whole stream. A segment that differs
    /// from the first segment of this render that was not taken from the cache, or from the
    /// first one at all, is deleted and marked to be rendered again. Returns whether any was.
    pub fn evict_mismatched(&mut self, rendered_now: &HashSet<PathBuf>) -> FFramesRendererResult<bool> {
        let reference = self
            .segments
            .iter()
            .find(|segment| rendered_now.contains(&segment.path))
            .or_else(|| self.segments.first())
            .map(|segment| segment.path.clone());
        let Some(reference) = reference else {
            return Ok(false);
        };
        let reference = codec_parameters(&reference);

        let mut evicted = false;
        for segment in &mut self.segments {
            if rendered_now.contains(&segment.path) {
                continue;
            }
            if codec_parameters(&segment.path) != reference {
                let _ = std::fs::remove_file(&segment.path);
                segment.cached = false;
                self.replaced += 1;
                evicted = true;
            }
        }
        Ok(evicted)
    }

    /// Renders the segments the cache does not have and stores them. `encode` renders the
    /// frames `0..sum` of segments with these lengths (the output frame of frame `i` is
    /// `frames[i]`) into segment files in `directory` and returns them in order.
    ///
    /// Cached segments with other codec parameters than the ones rendered now are rendered
    /// again. The frames taken from the cache count as rendered in the progress.
    pub fn render_stale(
        &mut self,
        logger: &Arc<dyn FFramesLogger>,
        mut encode: impl FnMut(&[usize], &[usize], &Path) -> FFramesRendererResult<Vec<PathBuf>>,
    ) -> FFramesRendererResult<SegmentCacheReport> {
        let planned = self.report();
        logger.log_segment_cache(&planned);
        for _ in 0..planned.reused_frames {
            logger.log_frame(0, 0);
        }

        let mut rendered_now = HashSet::new();
        for _ in 0..3 {
            if !self.is_complete() {
                let directory = self.render_directory()?;
                let paths: Vec<PathBuf> = self.stale().map(|segment| segment.path.clone()).collect();
                let files = encode(&self.stale_lengths(), &self.stale_frames(), directory.path())?;
                self.store(files)?;
                rendered_now.extend(paths);
            }
            if !self.evict_mismatched(&rendered_now)? {
                let report = SegmentCacheReport {
                    replaced_segments: self.replaced,
                    ..planned
                };
                if report.replaced_segments > 0 {
                    logger.log_segment_cache(&report);
                }
                return Ok(report);
            }
        }

        Err(FFramesRendererError::Internal(
            "segment cache: cached segments keep differing in codec parameters".to_owned(),
        ))
    }

    /// The segment files of the whole output in order.
    pub fn files(&self) -> Vec<PathBuf> {
        self.segments.iter().map(|segment| segment.path.clone()).collect()
    }

    /// What this render takes from the cache, before anything is rendered.
    pub fn report(&self) -> SegmentCacheReport {
        let reused: Vec<_> = self.segments.iter().filter(|segment| segment.cached).collect();
        SegmentCacheReport {
            segments: self.segments.len(),
            reused_segments: reused.len(),
            frames: self.segments.iter().map(|segment| segment.frames.len()).sum(),
            reused_frames: reused.iter().map(|segment| segment.frames.len()).sum(),
            fps: self.fps,
            replaced_segments: self.replaced,
        }
    }

    /// Marks the segments of this render as used and deletes the least recently used others
    /// until the cache fits [`SegmentCacheOptions::max_bytes`].
    pub fn finish(&self, logger: &Arc<dyn FFramesLogger>) {
        let now = SystemTime::now();
        let used: HashSet<&Path> = self
            .segments
            .iter()
            .map(|segment| segment.path.as_path())
            .collect();
        for path in &used {
            if let Ok(file) = std::fs::File::options().append(true).open(path) {
                let _ = file.set_modified(now);
            }
        }

        if let Err(err) = prune(&self.options, &used) {
            logger.warn(&format!("segment cache: could not prune: {err}"));
        }
    }
}

/// Render folders of renders that were killed are removed after this long.
const ABANDONED_RENDER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Deletes the least recently used segment files not in `keep` until the cache fits, and the
/// render folders of renders that were killed.
fn prune(options: &SegmentCacheOptions, keep: &HashSet<&Path>) -> std::io::Result<()> {
    let now = SystemTime::now();
    let mut files = Vec::new();
    let mut total = 0u64;
    for entry in std::fs::read_dir(options.segments_directory())? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let path = entry.path();
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        if metadata.is_dir() {
            let abandoned = now
                .duration_since(modified)
                .is_ok_and(|age| age > ABANDONED_RENDER);
            let render_folder = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("rendering-"));
            if render_folder && abandoned {
                std::fs::remove_dir_all(&path)?;
            }
            continue;
        }
        if !metadata.is_file() || !is_segment_file(&path) {
            continue;
        }
        total += metadata.len();
        files.push((modified, metadata.len(), path));
    }

    files.sort_by_key(|(modified, ..)| *modified);
    for (_, size, path) in files {
        if total <= options.max_bytes {
            break;
        }
        if keep.contains(path.as_path()) {
            continue;
        }
        std::fs::remove_file(&path)?;
        total -= size;
    }
    Ok(())
}
/// A file named `<64 hex digits>.<ext>`.
fn is_segment_file(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.len() == 64 && stem.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The scene starts of the video, in frames.
fn scene_starts(ctx: &FFramesContext) -> Vec<usize> {
    ctx.scenes
        .map(|scenes| scenes.iter().map(|(range, ..)| range.start).collect())
        .unwrap_or_default()
}

/// Cuts `output` at every scene start and then every `2 * fps` frames (a multiple of
/// `min_frames`) counted from the cut before. A piece shorter than `min_frames` joins the one
/// before it in the same scene. Cutting relative to scene starts keeps the segments of a scene
/// the same when an earlier scene gets longer or shorter.
fn cut(output: Range<usize>, scene_starts: &[usize], fps: usize, min_frames: usize) -> Vec<Range<usize>> {
    let min_frames = min_frames.max(1);
    let length = (2 * fps.max(1)).next_multiple_of(min_frames);

    let mut cuts: Vec<usize> = std::iter::once(output.start)
        .chain(
            scene_starts
                .iter()
                .copied()
                .filter(|start| output.contains(start)),
        )
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.push(output.end);

    let mut segments: Vec<Range<usize>> = Vec::new();
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let first = segments.len();
        let mut at = start;
        while at < end {
            let next = (at + length).min(end);
            if next - at < min_frames && segments.len() > first {
                segments.last_mut().expect("a piece of this scene").end = next;
            } else {
                segments.push(at..next);
            }
            at = next;
        }
    }
    segments
}

/// The pixels of the images the frames refer to by name (the media provider's images), hashed
/// once per render. Keyed by the address of their data, which lives for the whole render.
struct ImageHashes {
    /// Frames built as SVG text refer to images only by name, which `all` covers.
    #[cfg_attr(not(feature = "compile-time-svgtree"), allow(dead_code))]
    by_address: HashMap<(usize, usize), [u8; 32]>,
    /// Every named image: name, size and pixels.
    all: [u8; 32],
}

impl ImageHashes {
    fn new(images: Option<&HashMap<String, Arc<usvgr::PreloadedImageData>>>) -> Self {
        let mut by_address = HashMap::new();
        let mut named: Vec<(&String, [u8; 32], u32, u32)> = Vec::new();
        for (name, image) in images.into_iter().flatten() {
            let hash = *blake3::hash(&image.data).as_bytes();
            by_address.insert(address(image), hash);
            named.push((name, hash, image.width, image.height));
        }
        named.sort_unstable_by(|a, b| a.0.cmp(b.0));

        let mut hasher = blake3::Hasher::new();
        for (name, hash, width, height) in named {
            write_str(&mut hasher, name);
            hasher.update(&hash);
            hasher.update(&width.to_le_bytes());
            hasher.update(&height.to_le_bytes());
        }
        Self {
            by_address,
            all: *hasher.finalize().as_bytes(),
        }
    }

    #[cfg_attr(not(feature = "compile-time-svgtree"), allow(dead_code))]
    fn pixels(&self, image: &usvgr::PreloadedImageData) -> [u8; 32] {
        self.by_address
            .get(&address(image))
            .copied()
            .unwrap_or_else(|| *blake3::hash(&image.data).as_bytes())
    }
}

fn address(image: &usvgr::PreloadedImageData) -> (usize, usize) {
    (image.data.as_ptr() as usize, image.data.len())
}

/// The key of everything but the frames: the cache format, fframes, the codec libraries, the
/// output, the encoder, the conversion options, the fonts and the named images.
fn render_key(
    ctx: &FFramesContext,
    render_options: &RenderOptions,
    usvg_options: &usvgr::Options,
    font_db: &usvgr::fontdb::Database,
    images: &ImageHashes,
    key_input: &RenderKeyInput,
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(FORMAT);
    write_str(&mut h, env!("CARGO_PKG_VERSION"));
    unsafe {
        use crate::ffmpeg_sys_fframes::{avcodec_version, avformat_version};
        h.update(&avcodec_version().to_le_bytes());
        h.update(&avformat_version().to_le_bytes());
    }
    write_str(&mut h, key_input.renderer);
    write_str(&mut h, key_input.extension);

    let size = &ctx.current_video_size;
    h.update(&(size.width as u64).to_le_bytes());
    h.update(&(size.height as u64).to_le_bytes());
    h.update(&(ctx.time_base.fps as u64).to_le_bytes());
    write_str(&mut h, &format!("{:?}", render_options.video_encoder_options));

    write_str(&mut h, &usvg_options.font_family);
    h.update(&usvg_options.font_size.to_le_bytes());
    h.update(&usvg_options.dpi.to_le_bytes());
    h.update(&[u8::from(usvg_options.fast_shapes)]);
    for language in &usvg_options.languages {
        write_str(&mut h, language);
    }
    write_str(&mut h, usvg_options.style_sheet.as_deref().unwrap_or(""));
    write_str(
        &mut h,
        &format!(
            "{:?} {:?} {:?} {:?}",
            usvg_options.shape_rendering,
            usvg_options.text_rendering,
            usvg_options.image_rendering,
            usvg_options.default_size
        ),
    );

    h.update(&fonts_key(font_db));
    h.update(&images.all);
    *h.finalize().as_bytes()
}

/// Every font face: its names and style, and its data. Fonts in memory (the media provider's)
/// are hashed byte for byte, font files (system fonts) by path, size and modification time.
fn fonts_key(font_db: &usvgr::fontdb::Database) -> [u8; 32] {
    use usvgr::fontdb::Source;

    let mut faces: Vec<[u8; 32]> = font_db
        .faces()
        .map(|face| {
            let mut h = blake3::Hasher::new();
            write_str(
                &mut h,
                &format!(
                    "{:?} {} {:?} {:?} {:?} {} {}",
                    face.families,
                    face.post_script_name,
                    face.style,
                    face.weight,
                    face.stretch,
                    face.monospaced,
                    face.index
                ),
            );
            #[allow(unreachable_patterns)]
            match &face.source {
                Source::Binary(data) => {
                    h.update(b"binary");
                    h.update((**data).as_ref());
                }
                Source::File(path) => file_identity(&mut h, path),
                Source::SharedFile(path, _) => file_identity(&mut h, path),
                other => write_str(&mut h, &format!("{other:?}")),
            }
            *h.finalize().as_bytes()
        })
        .collect();
    faces.sort_unstable();

    let mut h = blake3::Hasher::new();
    for face in faces {
        h.update(&face);
    }
    *h.finalize().as_bytes()
}

fn file_identity(h: &mut blake3::Hasher, path: &Path) {
    h.update(b"file");
    write_str(h, &path.to_string_lossy());
    if let Ok(metadata) = std::fs::metadata(path) {
        h.update(&metadata.len().to_le_bytes());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_nanos());
        h.update(&modified.to_le_bytes());
    }
}

/// Builds every frame of `output` on all threads and hashes it.
fn frame_hashes<'a, 'media: 'a, TVideo: Video + Sync>(
    video: &'a TVideo,
    ctx: &FFramesContext<'a, 'media>,
    output: Range<usize>,
    usvg_options: &usvgr::Options,
    font_db: &usvgr::fontdb::Database,
    svgr_images: &SvgrImages,
    images: &ImageHashes,
) -> FFramesRendererResult<Vec<[u8; 32]>> {
    let threads = get_thread_count().clamp(1, output.len().max(1));
    let chunk = output.len().div_ceil(threads).max(1);

    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|thread| {
                let start = (output.start + thread * chunk).min(output.end);
                let end = (start + chunk).min(output.end);
                scope.spawn(move || {
                    svgr_images.enter(usvg_options, font_db, || {
                        // every thread builds a contiguous run, which keeps video decoding
                        // sequential
                        let text_cache = TextCache::new(10);
                        let decoders = VideoDecodersWorker::new(2);
                        (start..end)
                            .map(|frame| {
                                if ctx.abort_signal.is_some_and(crate::AbortSignal::is_aborted) {
                                    return Err(FFramesRendererError::Aborted);
                                }
                                let svgr = render_frame_guarded(
                                    video,
                                    Frame::__internal_make_for_renderer(
                                        frame,
                                        frame,
                                        ctx.time_base.fps,
                                        text_cache.clone(),
                                        decoders.clone(),
                                    ),
                                    ctx,
                                )?;
                                Ok(frame_hash(&svgr, images))
                            })
                            .collect::<FFramesRendererResult<Vec<_>>>()
                    })
                })
            })
            .collect();

        let mut hashes = Vec::with_capacity(output.len());
        for handle in handles {
            hashes.extend(handle.join().map_err(|_| {
                FFramesRendererError::Internal("frame hashing thread panicked".to_owned())
            })??);
        }
        Ok(hashes)
    })
}

/// The hash of everything a frame's tree says: elements, every attribute value in full (path
/// points, transforms, colours with alpha, lengths with units), text and embedded pixels.
fn frame_hash(svgr: &Svgr, images: &ImageHashes) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    hash_svgr(&mut h, svgr, images);
    *h.finalize().as_bytes()
}

#[cfg(feature = "compile-time-svgtree")]
fn hash_svgr(h: &mut blake3::Hasher, svgr: &Svgr, images: &ImageHashes) {
    h.update(b"tree");
    h.update(&(svgr.svg_tree.nodes.len() as u64).to_le_bytes());
    for node in &svgr.svg_tree.nodes {
        hash_optional_node(h, node.as_ref(), images);
    }
}

#[cfg(not(feature = "compile-time-svgtree"))]
fn hash_svgr(h: &mut blake3::Hasher, svgr: &Svgr, _images: &ImageHashes) {
    h.update(b"text");
    write_str(h, &svgr.value);
}

#[cfg(feature = "compile-time-svgtree")]
fn hash_optional_node(
    h: &mut blake3::Hasher,
    node: Option<&usvgr::svgtree::NestedNodeData>,
    images: &ImageHashes,
) {
    use usvgr::svgtree::NestedNodeKind;

    let Some(node) = node else {
        h.update(&[0]);
        return;
    };
    match &node.kind {
        NestedNodeKind::Root => {
            h.update(&[1]);
        }
        NestedNodeKind::Element { tag_name } => {
            h.update(&[2]);
            h.update(&(*tag_name as u16).to_le_bytes());
        }
        NestedNodeKind::Text(text) => {
            h.update(&[3]);
            write_str(h, text.as_str());
        }
    }

    h.update(&(node.attrs.len() as u64).to_le_bytes());
    for attribute in &node.attrs {
        h.update(&(attribute.name as u16).to_le_bytes());
        hash_value(h, &attribute.value, images);
    }

    h.update(&(node.children.len() as u64).to_le_bytes());
    for child in &node.children {
        hash_optional_node(h, child.as_ref(), images);
    }
}

#[cfg(feature = "compile-time-svgtree")]
fn hash_value(h: &mut blake3::Hasher, value: &usvgr::svgtree::SvgAttributeValue, images: &ImageHashes) {
    use usvgr::svgtree::SvgAttributeValue;

    match value {
        SvgAttributeValue::Float(number, text) => {
            h.update(&[0]);
            h.update(&number.to_le_bytes());
            write_str(h, text.as_str());
        }
        SvgAttributeValue::Length(length) => {
            h.update(&[1]);
            h.update(&length.number.to_le_bytes());
            h.update(&[length.unit as u8]);
        }
        SvgAttributeValue::Transform(ts) => {
            h.update(&[2]);
            for number in [ts.a, ts.b, ts.c, ts.d, ts.e, ts.f] {
                h.update(&number.to_le_bytes());
            }
        }
        SvgAttributeValue::Color(color) => {
            h.update(&[3, color.red, color.green, color.blue, color.alpha]);
        }
        SvgAttributeValue::StringStorage(text) => {
            h.update(&[4]);
            write_str(h, text.as_str());
        }
        SvgAttributeValue::ImageData(image) => {
            h.update(&[5]);
            write_str(h, &image.id);
            h.update(&image.width.to_le_bytes());
            h.update(&image.height.to_le_bytes());
            h.update(&images.pixels(image));
        }
        SvgAttributeValue::PathData(segments) => {
            h.update(&[6]);
            hash_segments(h, segments);
        }
        SvgAttributeValue::StaticPath(path) => {
            h.update(&[7]);
            hash_segments(h, path.segments());
        }
    }
}

#[cfg(feature = "compile-time-svgtree")]
fn hash_segments(h: &mut blake3::Hasher, segments: &[usvgr::svgtree::svgrtypes::PathSegment]) {
    use usvgr::svgtree::svgrtypes::PathSegment;

    h.update(&(segments.len() as u64).to_le_bytes());
    let numbers = |h: &mut blake3::Hasher, kind: u8, abs: bool, values: &[f64]| {
        h.update(&[kind, u8::from(abs)]);
        for value in values {
            h.update(&value.to_le_bytes());
        }
    };
    for segment in segments {
        match *segment {
            PathSegment::MoveTo { abs, x, y } => numbers(h, 0, abs, &[x, y]),
            PathSegment::LineTo { abs, x, y } => numbers(h, 1, abs, &[x, y]),
            PathSegment::HorizontalLineTo { abs, x } => numbers(h, 2, abs, &[x]),
            PathSegment::VerticalLineTo { abs, y } => numbers(h, 3, abs, &[y]),
            PathSegment::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => numbers(h, 4, abs, &[x1, y1, x2, y2, x, y]),
            PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                numbers(h, 5, abs, &[x2, y2, x, y]);
            }
            PathSegment::Quadratic { abs, x1, y1, x, y } => numbers(h, 6, abs, &[x1, y1, x, y]),
            PathSegment::SmoothQuadratic { abs, x, y } => numbers(h, 7, abs, &[x, y]),
            PathSegment::EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                numbers(h, 8, abs, &[rx, ry, x_axis_rotation, x, y]);
                h.update(&[u8::from(large_arc), u8::from(sweep)]);
            }
            PathSegment::ClosePath { abs } => numbers(h, 9, abs, &[]),
        }
    }
}

fn write_str(h: &mut blake3::Hasher, text: &str) {
    h.update(&(text.len() as u64).to_le_bytes());
    h.update(text.as_bytes());
}

/// The codec parameters of the video stream of a segment file (H.264: SPS and PPS), `None` if
/// it can not be read.
fn codec_parameters(path: &Path) -> Option<(i32, i32, i32, Vec<u8>)> {
    use crate::ffmpeg_sys_fframes::*;

    let file = std::ffi::CString::new(path.to_string_lossy().as_ref()).ok()?;
    unsafe {
        let mut input: *mut AVFormatContext = std::ptr::null_mut();
        if avformat_open_input(&raw mut input, file.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut()) < 0 {
            return None;
        }
        let streams = std::slice::from_raw_parts((*input).streams, (*input).nb_streams as usize);
        let parameters = streams.iter().find_map(|stream| {
            let codec = &*(**stream).codecpar;
            (codec.codec_type == AVMediaType::AVMEDIA_TYPE_VIDEO).then(|| {
                let extradata = if codec.extradata.is_null() || codec.extradata_size <= 0 {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(codec.extradata, codec.extradata_size as usize).to_vec()
                };
                (codec.codec_id as i32, codec.width, codec.height, extradata)
            })
        });
        avformat_close_input(&raw mut input);
        parameters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lengths(segments: &[Range<usize>]) -> Vec<usize> {
        segments.iter().map(ExactSizeIterator::len).collect()
    }

    #[test]
    fn cuts_at_scenes_and_every_two_seconds() {
        // 30 fps, GOP 24: segments of 72 frames counted from every scene start
        let segments = cut(0..300, &[0, 100, 250], 30, 24);
        assert_eq!(segments.first().unwrap().start, 0);
        assert_eq!(segments.last().unwrap().end, 300);
        assert!(segments.windows(2).all(|pair| pair[0].end == pair[1].start));
        assert!(segments.iter().any(|segment| segment.start == 100));
        assert!(segments.iter().any(|segment| segment.start == 250));
        // 0..100: 72 + 28; 100..250: 72 + 72 + 6 -> the 6 join the piece before
        assert_eq!(lengths(&segments), vec![72, 28, 72, 78, 50]);
    }

    #[test]
    fn a_longer_earlier_scene_keeps_the_later_segments() {
        let before = cut(0..400, &[0, 100, 200], 30, 24);
        let after = cut(0..430, &[0, 130, 230], 30, 24);
        let later = |segments: &[Range<usize>], start: usize| -> Vec<usize> {
            lengths(&segments.iter().filter(|segment| segment.start >= start).cloned().collect::<Vec<_>>())
        };
        assert_eq!(later(&before, 100), later(&after, 130));
    }

    #[test]
    fn a_range_starts_its_own_segment() {
        let segments = cut(50..120, &[0, 100], 30, 24);
        assert_eq!(segments, vec![50..100, 100..120]);
        assert_eq!(cut(0..10, &[], 30, 24), vec![0..10]);
        assert!(cut(5..5, &[], 30, 24).is_empty());
    }

    #[test]
    fn segment_file_names() {
        assert!(is_segment_file(Path::new(&format!("{}.mp4", "a".repeat(64)))));
        assert!(!is_segment_file(Path::new("0000000001.mp4")));
    }

    #[cfg(feature = "compile-time-svgtree")]
    #[test]
    fn path_points_change_the_hash() {
        use usvgr::svgtree::svgrtypes::PathSegment;
        let images = ImageHashes::new(None);
        let mut a = blake3::Hasher::new();
        hash_segments(&mut a, &[PathSegment::LineTo { abs: true, x: 1.0, y: 2.0 }]);
        let mut b = blake3::Hasher::new();
        hash_segments(&mut b, &[PathSegment::LineTo { abs: true, x: 1.0, y: 2.5 }]);
        assert_ne!(a.finalize(), b.finalize());
        drop(images);
    }

    #[test]
    fn same_tree_same_hash() {
        let images = ImageHashes::new(None);
        let a: Svgr = Svgr::from("<svg><rect width=\"10\" height=\"10\"/></svg>".to_owned());
        let b: Svgr = Svgr::from("<svg><rect width=\"10\" height=\"10\"/></svg>".to_owned());
        let c: Svgr = Svgr::from("<svg><rect width=\"10\" height=\"11\"/></svg>".to_owned());
        assert_eq!(frame_hash(&a, &images), frame_hash(&b, &images));
        assert_ne!(frame_hash(&a, &images), frame_hash(&c, &images));
    }
}

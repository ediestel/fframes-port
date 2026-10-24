#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColorDepth {
    Bits8,
    Bits10,
    Bits12,
    Bits16,
}

impl ColorDepth {
    pub const fn bits(&self) -> u8 {
        match self {
            ColorDepth::Bits8 => 8,
            ColorDepth::Bits10 => 10,
            ColorDepth::Bits12 => 12,
            ColorDepth::Bits16 => 16,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Yuv420P,
    Yuv420P10Le,
    Yuv444P,
    Yuv444P10Le,
    Yuva444P10Le,
    Rgba8,
    Rgba16,
    ExrHalf,
}

impl PixelFormat {
    pub const fn bit_depth(&self) -> u8 {
        match self {
            PixelFormat::Yuv420P | PixelFormat::Yuv444P | PixelFormat::Rgba8 => 8,
            PixelFormat::Yuv420P10Le | PixelFormat::Yuv444P10Le | PixelFormat::Yuva444P10Le => 10,
            PixelFormat::Rgba16 => 16,
            PixelFormat::ExrHalf => 16,
        }
    }

    pub const fn has_alpha(&self) -> bool {
        matches!(
            self,
            PixelFormat::Yuva444P10Le
                | PixelFormat::Rgba8
                | PixelFormat::Rgba16
                | PixelFormat::ExrHalf
        )
    }

    /// Bytes of one `width` x `height` frame. A 4:2:0 frame has one chroma sample per 2x2 block
    /// (1.5 samples per pixel), so its size is not a whole number of bytes per pixel.
    pub const fn frame_bytes(&self, width: usize, height: usize) -> usize {
        let pixels = width * height;
        let chroma_420 = width.div_ceil(2) * height.div_ceil(2);
        match self {
            PixelFormat::Rgba8 => pixels * 4,
            PixelFormat::Rgba16 => pixels * 8,
            PixelFormat::ExrHalf => pixels * 8,
            PixelFormat::Yuv420P => pixels + 2 * chroma_420,
            PixelFormat::Yuv420P10Le => 2 * (pixels + 2 * chroma_420),
            PixelFormat::Yuv444P => pixels * 3,
            PixelFormat::Yuv444P10Le => pixels * 6,
            PixelFormat::Yuva444P10Le => pixels * 8,
        }
    }

    pub const fn color_depth(&self) -> ColorDepth {
        match self {
            PixelFormat::Yuv420P | PixelFormat::Yuv444P | PixelFormat::Rgba8 => ColorDepth::Bits8,
            PixelFormat::Yuv420P10Le | PixelFormat::Yuv444P10Le | PixelFormat::Yuva444P10Le => {
                ColorDepth::Bits10
            }
            PixelFormat::Rgba16 | PixelFormat::ExrHalf => ColorDepth::Bits16,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PixelFormat {
    pub fn to_av_pixel_format(&self) -> crate::ffmpeg_sys_fframes::AVPixelFormat {
        use crate::ffmpeg_sys_fframes::AVPixelFormat;
        match self {
            PixelFormat::Yuv420P => AVPixelFormat::AV_PIX_FMT_YUV420P,
            PixelFormat::Yuv420P10Le => AVPixelFormat::AV_PIX_FMT_YUV420P10LE,
            PixelFormat::Yuv444P => AVPixelFormat::AV_PIX_FMT_YUV444P,
            PixelFormat::Yuv444P10Le => AVPixelFormat::AV_PIX_FMT_YUV444P10LE,
            PixelFormat::Yuva444P10Le => AVPixelFormat::AV_PIX_FMT_YUVA444P10LE,
            PixelFormat::Rgba8 => AVPixelFormat::AV_PIX_FMT_RGBA,
            PixelFormat::Rgba16 => AVPixelFormat::AV_PIX_FMT_RGBA64LE,
            PixelFormat::ExrHalf => AVPixelFormat::AV_PIX_FMT_GBRAPF32LE,
        }
    }

    pub fn from_av_pixel_format(fmt: crate::ffmpeg_sys_fframes::AVPixelFormat) -> Option<Self> {
        use crate::ffmpeg_sys_fframes::AVPixelFormat;
        match fmt {
            AVPixelFormat::AV_PIX_FMT_YUV420P => Some(PixelFormat::Yuv420P),
            AVPixelFormat::AV_PIX_FMT_YUV420P10LE => Some(PixelFormat::Yuv420P10Le),
            AVPixelFormat::AV_PIX_FMT_YUV444P => Some(PixelFormat::Yuv444P),
            AVPixelFormat::AV_PIX_FMT_YUV444P10LE => Some(PixelFormat::Yuv444P10Le),
            AVPixelFormat::AV_PIX_FMT_YUVA444P10LE => Some(PixelFormat::Yuva444P10Le),
            AVPixelFormat::AV_PIX_FMT_RGBA => Some(PixelFormat::Rgba8),
            AVPixelFormat::AV_PIX_FMT_RGBA64LE => Some(PixelFormat::Rgba16),
            AVPixelFormat::AV_PIX_FMT_GBRAPF32LE => Some(PixelFormat::ExrHalf),
            _ => None,
        }
    }

    pub fn av_pixel_format_has_alpha(fmt: crate::ffmpeg_sys_fframes::AVPixelFormat) -> bool {
        Self::from_av_pixel_format(fmt)
            .map(|pf| pf.has_alpha())
            .unwrap_or(false)
    }
}

/// Render-time pixel substrate. Distinct from `PixelFormat` (which
/// describes the OUTPUT pixel layout). `RenderSubstrate` describes the
/// IN-MEMORY pixmap the renderer paints into before the YUV conversion.
///
/// `Bits8` matches the legacy tiny_skia pixmap path (8-bit RGBA, one
/// sample per output pixel). `Bits16` enables a supersample-then-
/// downsample path: the SVG is rasterised at 2x2 the output resolution
/// into an 8-bit pixmap, then gamma-correctly averaged down into a
/// u16 RGBA buffer before the YUV conversion. The 4-sample average
/// yields more-than-8-bit effective precision (roughly log2(4) ≈ 2
/// extra bits), so a 10-bit output (Yuv420P10Le, ProRes) carries real
/// extra information instead of `u << 2` bit padding.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum RenderSubstrate {
    #[default]
    Bits8,
    Bits16,
}

impl RenderSubstrate {
    /// All substrates are wired end-to-end. Kept as a method for
    /// call-site symmetry with older versions that returned `false`
    /// for `Bits16`.
    pub const fn is_supported(&self) -> bool {
        true
    }

    pub const fn bytes_per_pixel(&self) -> usize {
        match self {
            RenderSubstrate::Bits8 => 4,
            RenderSubstrate::Bits16 => 8,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum AlphaMode {
    #[default]
    None,
    Straight,
    Premultiplied,
}

impl AlphaMode {
    pub const fn carries_alpha(&self) -> bool {
        !matches!(self, AlphaMode::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlphaConfigError {
    IncompatibleAlphaMode {
        alpha_mode: AlphaMode,
        pixel_format: PixelFormat,
    },
}

impl std::fmt::Display for AlphaConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlphaConfigError::IncompatibleAlphaMode {
                alpha_mode,
                pixel_format,
            } => write!(
                f,
                "AlphaMode {alpha_mode:?} requires a pixel format with alpha, but {pixel_format:?} has no alpha channel"
            ),
        }
    }
}

impl std::error::Error for AlphaConfigError {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AlphaAwarePixelConfig {
    pub pixel_format: PixelFormat,
    pub alpha_mode: AlphaMode,
    pub color_space: crate::color_space::ColorSpaceDescriptor,
}

impl AlphaAwarePixelConfig {
    pub fn new(
        pixel_format: PixelFormat,
        alpha_mode: AlphaMode,
        color_space: crate::color_space::ColorSpaceDescriptor,
    ) -> Result<Self, AlphaConfigError> {
        if alpha_mode.carries_alpha() && !pixel_format.has_alpha() {
            return Err(AlphaConfigError::IncompatibleAlphaMode {
                alpha_mode,
                pixel_format,
            });
        }
        Ok(Self {
            pixel_format,
            alpha_mode,
            color_space,
        })
    }

    pub const fn opaque(
        pixel_format: PixelFormat,
        color_space: crate::color_space::ColorSpaceDescriptor,
    ) -> Self {
        Self {
            pixel_format,
            alpha_mode: AlphaMode::None,
            color_space,
        }
    }

    pub const fn default_sdr() -> Self {
        Self {
            pixel_format: PixelFormat::Yuv420P,
            alpha_mode: AlphaMode::None,
            color_space: crate::color_space::ColorSpaceDescriptor::sdr_bt709(),
        }
    }
}

impl Default for AlphaAwarePixelConfig {
    fn default() -> Self {
        Self::default_sdr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_bits() {
        assert_eq!(ColorDepth::Bits8.bits(), 8);
        assert_eq!(ColorDepth::Bits10.bits(), 10);
        assert_eq!(ColorDepth::Bits12.bits(), 12);
        assert_eq!(ColorDepth::Bits16.bits(), 16);
    }

    #[test]
    fn format_properties() {
        assert_eq!(PixelFormat::Yuv420P.bit_depth(), 8);
        assert_eq!(PixelFormat::Yuv420P10Le.bit_depth(), 10);
        assert!(PixelFormat::Yuva444P10Le.has_alpha());
        assert!(!PixelFormat::Yuv420P.has_alpha());
        assert_eq!(PixelFormat::Rgba8.frame_bytes(2, 2), 16);
        assert_eq!(PixelFormat::Yuva444P10Le.color_depth(), ColorDepth::Bits10);
    }

    #[test]
    fn yuv420_frame_bytes() {
        assert_eq!(PixelFormat::Yuv420P.frame_bytes(1920, 1080), 1920 * 1080 * 3 / 2);
        assert_eq!(PixelFormat::Yuv420P10Le.frame_bytes(1920, 1080), 1920 * 1080 * 3);
        // An odd size rounds the chroma planes up: 3x3 luma, 2x2 for each chroma plane.
        assert_eq!(PixelFormat::Yuv420P.frame_bytes(3, 3), 9 + 2 * 4);
    }

    #[test]
    fn render_substrate_support() {
        assert!(RenderSubstrate::Bits8.is_supported());
        assert!(RenderSubstrate::Bits16.is_supported());
        assert_eq!(RenderSubstrate::Bits8.bytes_per_pixel(), 4);
        assert_eq!(RenderSubstrate::Bits16.bytes_per_pixel(), 8);
    }

    #[test]
    fn alpha_mode_defaults() {
        assert_eq!(AlphaMode::default(), AlphaMode::None);
        assert!(AlphaMode::Straight.carries_alpha());
        assert!(AlphaMode::Premultiplied.carries_alpha());
        assert!(!AlphaMode::None.carries_alpha());
    }

    #[test]
    fn alpha_config_rejects_straight_on_opaque_format() {
        let err = AlphaAwarePixelConfig::new(
            PixelFormat::Yuv420P,
            AlphaMode::Straight,
            crate::color_space::ColorSpaceDescriptor::sdr_bt709(),
        );
        assert!(err.is_err());
    }

    #[test]
    fn alpha_config_accepts_none_on_opaque_format() {
        let ok = AlphaAwarePixelConfig::new(
            PixelFormat::Yuv420P,
            AlphaMode::None,
            crate::color_space::ColorSpaceDescriptor::sdr_bt709(),
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn alpha_config_accepts_straight_on_alpha_format() {
        let ok = AlphaAwarePixelConfig::new(
            PixelFormat::Yuva444P10Le,
            AlphaMode::Straight,
            crate::color_space::ColorSpaceDescriptor::sdr_bt709(),
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn alpha_config_opaque_convenience() {
        let cfg = AlphaAwarePixelConfig::opaque(
            PixelFormat::Yuv420P10Le,
            crate::color_space::ColorSpaceDescriptor::hdr_bt2020_pq(),
        );
        assert_eq!(cfg.alpha_mode, AlphaMode::None);
        assert_eq!(cfg.pixel_format, PixelFormat::Yuv420P10Le);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround51,
    Surround71,
    Atmos,
}

impl ChannelLayout {
    pub const fn channel_count(&self) -> u8 {
        match self {
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
            ChannelLayout::Surround51 => 6,
            ChannelLayout::Surround71 => 8,
            ChannelLayout::Atmos => 16,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ChannelLayout {
    pub fn to_av_channel_layout(&self) -> crate::ffmpeg_sys_fframes::AVChannelLayout {
        use crate::ffmpeg_sys_fframes::{
            AV_CH_LAYOUT_5POINT1, AV_CH_LAYOUT_7POINT1, AV_CH_LAYOUT_MONO, AV_CH_LAYOUT_STEREO,
            AVChannelLayout, AVChannelLayout__bindgen_ty_1, AVChannelOrder,
        };
        let (nb_channels, mask) = match self {
            ChannelLayout::Mono => (1, AV_CH_LAYOUT_MONO),
            ChannelLayout::Stereo => (2, AV_CH_LAYOUT_STEREO),
            ChannelLayout::Surround51 => (6, AV_CH_LAYOUT_5POINT1),
            ChannelLayout::Surround71 => (8, AV_CH_LAYOUT_7POINT1),
            ChannelLayout::Atmos => (8, AV_CH_LAYOUT_7POINT1),
        };
        AVChannelLayout {
            order: AVChannelOrder::AV_CHANNEL_ORDER_NATIVE,
            nb_channels,
            u: AVChannelLayout__bindgen_ty_1 { mask },
            opaque: std::ptr::null_mut(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum AudioBitDepth {
    Int16,
    Int24,
    #[default]
    Float32,
    Float64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BwfMetadata {
    pub description: Option<String>,
    pub originator: Option<String>,
    pub originator_reference: Option<String>,
    pub origination_date: Option<String>,
    pub origination_time: Option<String>,
    pub time_reference: Option<u64>,
}

impl Default for BwfMetadata {
    fn default() -> Self {
        Self {
            description: None,
            originator: Some("fframes".to_string()),
            originator_reference: None,
            origination_date: None,
            origination_time: None,
            time_reference: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub bit_depth: AudioBitDepth,
    pub layout: ChannelLayout,
    pub lufs_target: Option<f32>,
    pub bwf_metadata: Option<BwfMetadata>,
}

impl Default for AudioSpec {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            bit_depth: AudioBitDepth::Float32,
            layout: ChannelLayout::Mono,
            lufs_target: None,
            bwf_metadata: None,
        }
    }
}

impl AudioSpec {
    pub const fn mono() -> Self {
        Self {
            sample_rate: 44_100,
            bit_depth: AudioBitDepth::Float32,
            layout: ChannelLayout::Mono,
            lufs_target: None,
            bwf_metadata: None,
        }
    }

    pub const fn stereo_48k() -> Self {
        Self {
            sample_rate: 48_000,
            bit_depth: AudioBitDepth::Float32,
            layout: ChannelLayout::Stereo,
            lufs_target: None,
            bwf_metadata: None,
        }
    }

    pub const fn broadcast_51() -> Self {
        Self {
            sample_rate: 48_000,
            bit_depth: AudioBitDepth::Int24,
            layout: ChannelLayout::Surround51,
            lufs_target: Some(-23.0),
            bwf_metadata: None,
        }
    }

    pub fn with_lufs(mut self, lufs: f32) -> Self {
        self.lufs_target = Some(lufs);
        self
    }

    pub fn with_bwf(mut self, metadata: BwfMetadata) -> Self {
        self.bwf_metadata = Some(metadata);
        self
    }
}

/// Linear gain that moves a signal measured at `current_lufs` to `target_lufs`. Measure the
/// loudness with [`crate::LoudnessAnalysis`] or [`crate::analyze_audio`] (ITU-R BS.1770 / EBU R128).
pub fn lufs_normalization_gain(current_lufs: f32, target_lufs: f32) -> f32 {
    let db = target_lufs - current_lufs;
    10.0_f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_channels() {
        assert_eq!(ChannelLayout::Mono.channel_count(), 1);
        assert_eq!(ChannelLayout::Stereo.channel_count(), 2);
        assert_eq!(ChannelLayout::Surround51.channel_count(), 6);
        assert_eq!(ChannelLayout::Surround71.channel_count(), 8);
    }

    #[test]
    fn defaults() {
        let s = AudioSpec::default();
        assert_eq!(s.sample_rate, 44_100);
        assert_eq!(s.layout, ChannelLayout::Mono);
    }

    #[test]
    fn lufs_gain() {
        let gain = lufs_normalization_gain(-20.0, -23.0);
        assert!((gain - 0.7079).abs() < 0.001);
    }
}

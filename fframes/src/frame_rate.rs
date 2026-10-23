#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FrameRate {
    pub num: u32,
    pub den: u32,
    pub drop_frame: bool,
}

impl FrameRate {
    pub const fn new(num: u32, den: u32) -> Self {
        Self {
            num,
            den,
            drop_frame: false,
        }
    }

    pub const fn exact(fps: u32) -> Self {
        Self {
            num: fps,
            den: 1,
            drop_frame: false,
        }
    }

    pub const fn ntsc_29_97() -> Self {
        Self {
            num: 30_000,
            den: 1_001,
            drop_frame: true,
        }
    }

    pub const fn ntsc_59_94() -> Self {
        Self {
            num: 60_000,
            den: 1_001,
            drop_frame: true,
        }
    }

    pub const fn film_24() -> Self {
        Self {
            num: 24,
            den: 1,
            drop_frame: false,
        }
    }

    pub const fn film_23_976() -> Self {
        Self {
            num: 24_000,
            den: 1_001,
            drop_frame: false,
        }
    }

    pub const fn cinema_48() -> Self {
        Self {
            num: 48,
            den: 1,
            drop_frame: false,
        }
    }

    pub const fn pal_25() -> Self {
        Self {
            num: 25,
            den: 1,
            drop_frame: false,
        }
    }

    pub fn as_f64(&self) -> f64 {
        self.num as f64 / self.den as f64
    }

    pub fn as_f32(&self) -> f32 {
        self.num as f32 / self.den as f32
    }

    pub fn rounded(&self) -> u32 {
        (self.as_f64() + 0.5) as u32
    }

    pub fn time_base_num(&self) -> u32 {
        self.den
    }

    pub fn time_base_den(&self) -> u32 {
        self.num
    }

    pub fn pts_for_frame(&self, frame_index: u64, time_base_den: u64) -> i64 {
        let num = self.num as u64;
        let den = self.den as u64;
        let pts = (frame_index as u128 * time_base_den as u128 * den as u128) / num as u128;
        pts as i64
    }

    pub fn is_drop_frame(&self) -> bool {
        self.drop_frame
    }

    pub fn is_integer(&self) -> bool {
        self.den == 1
    }

    pub fn vfr_tolerance_ticks(&self) -> u64 {
        1
    }

    /// Whether a stream advertised at this frame rate should be suspected
    /// of being variable-frame-rate (VFR). The renderer treats integer
    /// rates and the standard NTSC rationals as constant; anything else
    /// is flagged as a VFR candidate so `EncoderOptions.enforce_cfr` can
    /// reject it at stream validation time.
    pub fn is_vfr_candidate(&self) -> bool {
        if self.is_integer() {
            return false;
        }
        !matches!(
            (self.num, self.den),
            (24_000, 1_001) | (30_000, 1_001) | (60_000, 1_001)
        )
    }
}

impl From<u32> for FrameRate {
    fn from(fps: u32) -> Self {
        Self::exact(fps)
    }
}

impl From<usize> for FrameRate {
    fn from(fps: usize) -> Self {
        Self::exact(fps as u32)
    }
}

impl std::fmt::Display for FrameRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_integer() {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{} ({:.3})", self.num, self.den, self.as_f64())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_rates() {
        assert_eq!(FrameRate::exact(30).as_f64(), 30.0);
        assert_eq!(FrameRate::film_24().rounded(), 24);
        assert!(FrameRate::film_24().is_integer());
    }

    #[test]
    fn ntsc_precision() {
        let r = FrameRate::ntsc_29_97();
        assert!((r.as_f64() - 29.97002997).abs() < 1e-6);
        assert!(r.is_drop_frame());
        assert!(!r.is_integer());
        assert_eq!(r.rounded(), 30);
    }

    #[test]
    fn pts_calculation_integer() {
        let r = FrameRate::exact(30);
        assert_eq!(r.pts_for_frame(0, 30), 0);
        assert_eq!(r.pts_for_frame(30, 30), 30);
    }

    #[test]
    fn pts_calculation_ntsc() {
        let r = FrameRate::ntsc_29_97();
        let tb_den = 30_000_u64;
        assert_eq!(r.pts_for_frame(0, tb_den), 0);
        assert_eq!(r.pts_for_frame(30, tb_den), 30 * 1001);
    }

    #[test]
    fn from_u32_defaults_to_integer() {
        let r: FrameRate = 60_u32.into();
        assert_eq!(r.num, 60);
        assert_eq!(r.den, 1);
        assert!(!r.drop_frame);
    }
}

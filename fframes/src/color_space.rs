#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColorPrimaries {
    Bt709,
    Bt2020,
    DciP3,
    Srgb,
    Bt601Pal,
    Bt601Ntsc,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransferCharacteristics {
    Bt709,
    Pq,
    Hlg,
    Linear,
    Srgb,
    Bt601,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MatrixCoefficients {
    Bt709,
    Bt2020Ncl,
    Bt601,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColorRange {
    Limited,
    Full,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ColorSpaceDescriptor {
    pub primaries: ColorPrimaries,
    pub transfer: TransferCharacteristics,
    pub matrix: MatrixCoefficients,
    pub range: ColorRange,
}

impl ColorSpaceDescriptor {
    pub const fn sdr_bt709() -> Self {
        Self {
            primaries: ColorPrimaries::Bt709,
            transfer: TransferCharacteristics::Bt709,
            matrix: MatrixCoefficients::Bt709,
            range: ColorRange::Limited,
        }
    }

    pub const fn sdr_bt601() -> Self {
        Self {
            primaries: ColorPrimaries::Bt601Ntsc,
            transfer: TransferCharacteristics::Bt601,
            matrix: MatrixCoefficients::Bt601,
            range: ColorRange::Limited,
        }
    }

    pub const fn hdr_bt2020_pq() -> Self {
        Self {
            primaries: ColorPrimaries::Bt2020,
            transfer: TransferCharacteristics::Pq,
            matrix: MatrixCoefficients::Bt2020Ncl,
            range: ColorRange::Limited,
        }
    }

    pub const fn hdr_bt2020_hlg() -> Self {
        Self {
            primaries: ColorPrimaries::Bt2020,
            transfer: TransferCharacteristics::Hlg,
            matrix: MatrixCoefficients::Bt2020Ncl,
            range: ColorRange::Limited,
        }
    }

    pub const fn srgb() -> Self {
        Self {
            primaries: ColorPrimaries::Srgb,
            transfer: TransferCharacteristics::Srgb,
            matrix: MatrixCoefficients::Bt709,
            range: ColorRange::Full,
        }
    }
}

impl Default for ColorSpaceDescriptor {
    fn default() -> Self {
        Self::sdr_bt709()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ColorSpaceDescriptor {
    pub fn to_av_color_primaries(&self) -> crate::ffmpeg_sys_fframes::AVColorPrimaries {
        use crate::ffmpeg_sys_fframes::AVColorPrimaries;
        match self.primaries {
            ColorPrimaries::Bt709 => AVColorPrimaries::AVCOL_PRI_BT709,
            ColorPrimaries::Bt2020 => AVColorPrimaries::AVCOL_PRI_BT2020,
            ColorPrimaries::DciP3 => AVColorPrimaries::AVCOL_PRI_SMPTE432,
            ColorPrimaries::Srgb => AVColorPrimaries::AVCOL_PRI_BT709,
            ColorPrimaries::Bt601Pal => AVColorPrimaries::AVCOL_PRI_BT470BG,
            ColorPrimaries::Bt601Ntsc => AVColorPrimaries::AVCOL_PRI_SMPTE170M,
        }
    }

    pub fn to_av_color_trc(&self) -> crate::ffmpeg_sys_fframes::AVColorTransferCharacteristic {
        use crate::ffmpeg_sys_fframes::AVColorTransferCharacteristic;
        match self.transfer {
            TransferCharacteristics::Bt709 => AVColorTransferCharacteristic::AVCOL_TRC_BT709,
            TransferCharacteristics::Pq => AVColorTransferCharacteristic::AVCOL_TRC_SMPTE2084,
            TransferCharacteristics::Hlg => AVColorTransferCharacteristic::AVCOL_TRC_ARIB_STD_B67,
            TransferCharacteristics::Linear => AVColorTransferCharacteristic::AVCOL_TRC_LINEAR,
            TransferCharacteristics::Srgb => AVColorTransferCharacteristic::AVCOL_TRC_IEC61966_2_1,
            TransferCharacteristics::Bt601 => AVColorTransferCharacteristic::AVCOL_TRC_SMPTE170M,
        }
    }

    pub fn to_av_colorspace(&self) -> crate::ffmpeg_sys_fframes::AVColorSpace {
        use crate::ffmpeg_sys_fframes::AVColorSpace;
        match self.matrix {
            MatrixCoefficients::Bt709 => AVColorSpace::AVCOL_SPC_BT709,
            MatrixCoefficients::Bt2020Ncl => AVColorSpace::AVCOL_SPC_BT2020_NCL,
            MatrixCoefficients::Bt601 => AVColorSpace::AVCOL_SPC_SMPTE170M,
        }
    }

    pub fn to_av_color_range(&self) -> crate::ffmpeg_sys_fframes::AVColorRange {
        use crate::ffmpeg_sys_fframes::AVColorRange;
        match self.range {
            ColorRange::Limited => AVColorRange::AVCOL_RANGE_MPEG,
            ColorRange::Full => AVColorRange::AVCOL_RANGE_JPEG,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ColorMatrix {
    pub ky_r: i32,
    pub ky_g: i32,
    pub ky_b: i32,
    pub kcb_r: i32,
    pub kcb_g: i32,
    pub kcb_b: i32,
    pub kcr_r: i32,
    pub kcr_g: i32,
    pub kcr_b: i32,
    pub y_offset: i32,
    pub c_offset: i32,
    pub shift: u32,
}

impl ColorMatrix {
    pub const BT601: ColorMatrix = ColorMatrix {
        ky_r: 66,
        ky_g: 129,
        ky_b: 25,
        kcb_r: -38,
        kcb_g: -74,
        kcb_b: 112,
        kcr_r: 112,
        kcr_g: -94,
        kcr_b: -18,
        y_offset: 16,
        c_offset: 128,
        shift: 8,
    };

    pub const BT709: ColorMatrix = ColorMatrix {
        ky_r: 47,
        ky_g: 157,
        ky_b: 16,
        kcb_r: -26,
        kcb_g: -87,
        kcb_b: 112,
        kcr_r: 112,
        kcr_g: -102,
        kcr_b: -10,
        y_offset: 16,
        c_offset: 128,
        shift: 8,
    };

    pub const BT2020: ColorMatrix = ColorMatrix {
        ky_r: 57,
        ky_g: 147,
        ky_b: 20,
        kcb_r: -31,
        kcb_g: -81,
        kcb_b: 112,
        kcr_r: 112,
        kcr_g: -103,
        kcr_b: -9,
        y_offset: 16,
        c_offset: 128,
        shift: 8,
    };

    pub const fn for_coefficients(matrix: MatrixCoefficients) -> ColorMatrix {
        match matrix {
            MatrixCoefficients::Bt601 => Self::BT601,
            MatrixCoefficients::Bt709 => Self::BT709,
            MatrixCoefficients::Bt2020Ncl => Self::BT2020,
        }
    }

    pub const fn scale_to_bits(&self, target_bits: u8) -> ColorMatrix {
        let src_bits = 8_i32;
        let shift = target_bits as i32 - src_bits;
        if shift <= 0 {
            return *self;
        }
        ColorMatrix {
            ky_r: self.ky_r,
            ky_g: self.ky_g,
            ky_b: self.ky_b,
            kcb_r: self.kcb_r,
            kcb_g: self.kcb_g,
            kcb_b: self.kcb_b,
            kcr_r: self.kcr_r,
            kcr_g: self.kcr_g,
            kcr_b: self.kcr_b,
            y_offset: self.y_offset << shift,
            c_offset: self.c_offset << shift,
            shift: self.shift,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_bt709() {
        let cs = ColorSpaceDescriptor::default();
        assert_eq!(cs.primaries, ColorPrimaries::Bt709);
        assert_eq!(cs.matrix, MatrixCoefficients::Bt709);
    }

    #[test]
    fn matrix_select() {
        let bt709 = ColorMatrix::for_coefficients(MatrixCoefficients::Bt709);
        assert_eq!(bt709.ky_r, 47);
        assert_eq!(bt709.ky_g, 157);
        let bt601 = ColorMatrix::for_coefficients(MatrixCoefficients::Bt601);
        assert_eq!(bt601.ky_r, 66);
    }

    #[test]
    fn rgb_to_y_bt709() {
        let m = ColorMatrix::BT709;
        let r = 255_i32;
        let g = 255_i32;
        let b = 255_i32;
        let y = (m.y_offset + ((m.ky_r * r + m.ky_g * g + m.ky_b * b) >> m.shift)) as u8;
        assert!((230..=240).contains(&y), "white Y should be ~235, got {y}");
    }
}

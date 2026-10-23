//! `RgbaFrameConverter` gets premultiplied pixels from the rasterizers. Formats with an alpha
//! channel must receive straight colors (swscale reads RGBA as straight), formats without one
//! keep the premultiplied colors: a transparent pixel over black once the alpha is dropped.

use crate::RgbaFrameConverter;
use crate::media::ffmpeg_sys_fframes::AVPixelFormat;

const SIZE: i32 = 8;

/// The first sample of every plane after converting a frame filled with `rgba`.
fn convert(format: AVPixelFormat, rgba: [u8; 4], planes: usize) -> Vec<u8> {
    let mut converter = RgbaFrameConverter::new(format, SIZE, SIZE).unwrap();
    let pixels = rgba.repeat((SIZE * SIZE) as usize);
    let frame = converter.convert(&pixels).unwrap();
    (0..planes)
        .map(|plane| frame.copy_plane(plane, 1, 1)[0])
        .collect()
}

#[test]
fn rgba_output_gets_straight_alpha() {
    let mut converter =
        RgbaFrameConverter::new(AVPixelFormat::AV_PIX_FMT_RGBA, SIZE, SIZE).unwrap();
    let half_red_premultiplied = [128, 0, 0, 128].repeat((SIZE * SIZE) as usize);
    let frame = converter.convert(&half_red_premultiplied).unwrap();
    assert_eq!(frame.copy_plane(0, 4, 1), vec![255, 0, 0, 128]);
}

#[test]
fn yuva_output_has_the_color_of_the_opaque_pixel_and_its_alpha() {
    let translucent = convert(AVPixelFormat::AV_PIX_FMT_YUVA444P, [128, 0, 0, 128], 4);
    let opaque = convert(AVPixelFormat::AV_PIX_FMT_YUVA444P, [255, 0, 0, 255], 4);
    assert_eq!(
        translucent[..3],
        opaque[..3],
        "Y, Cb, Cr of half transparent red"
    );
    // swscale rounds the alpha plane itself; the fix only touches the colors.
    assert!(
        translucent[3].abs_diff(128) <= 1,
        "alpha plane {}",
        translucent[3]
    );
}

#[test]
fn output_without_alpha_keeps_premultiplied_colors() {
    let translucent = convert(AVPixelFormat::AV_PIX_FMT_YUV444P, [128, 0, 0, 128], 3);
    let dark_red = convert(AVPixelFormat::AV_PIX_FMT_YUV444P, [128, 0, 0, 255], 3);
    assert_eq!(translucent, dark_red);
}

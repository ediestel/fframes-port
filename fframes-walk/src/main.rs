//! `fframes-walk --model window.glb[:wall[:height[:sill[:offset]]]] [--window ...] [--room W,D,H]
//! [--quality 0.5]`
//!
//! Walk through a room with glTF models in it. The room (metres) has a polished stone floor
//! that mirrors, stone walls and a ceiling. `--model` puts a model on a wall (`back`,
//! `front`, `left`, `right`), `height` metres high with its lower edge `sill` above the
//! floor; `--window` does the same and cuts an opening for it, its colours glow as if lit from
//! behind and the sun shines in through it. Rendered with `fframes-three-d` on the GPU into the
//! native player's window.
//!
//! `--video out.mp4 [--seconds 15] [--size 1920x1080]` renders a walk from the start up to the
//! first window, ending with a look up at it, and encodes it with the `ffmpeg` command.
//! `--demo` instead plays a scripted walk with the controls drawn top right (W A S D, Shift,
//! the mouse), lit while held, at the speeds of the window. `--captions captions.txt` writes
//! text over it: one caption per line, `start-end|line|line`
//! in seconds, for example `0.5-3.5|A stained-glass window|6,765 pieces`; each line after the
//! first is smaller.
//! `--at x,z` starts there instead of near the front wall. `--snapshot out.png` renders one 1280 x 800 picture from the starting point and quits.
//!
//! Walk: `W` `A` `S` `D` or the arrow keys, `Shift` faster. Look: drag with the mouse or
//! scroll with two fingers. `R` back to the start, `Q` / `Esc` quit.

mod room;

use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

use fframes_native_player::fframes_skia_renderer::skia_safe::{
    self, AlphaType, Color, ColorType, Data, Font, FontMgr, FontStyle, ImageInfo, Paint, Rect,
    SamplingOptions,
};
use fframes_native_player::winit;
use fframes_native_player::{PlayerBackend, Surface};
use fframes_three_d::{Bloom, Camera, Renderer, Vec3, vec3};
use room::{Placement, RoomSize, Wall};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const SYSTEM_FONTS: &[&str] = &[".AppleSystemUIFont", "Helvetica Neue", "Segoe UI", "DejaVu Sans"];

fn usage() -> &'static str {
    "fframes-walk --window window.glb[:wall[:height[:sill[:offset]]]] [--model m.glb[:...]] [--room W,D,H] [--glow 2] [--quality 0.5]"
}

/// `path[:wall[:height[:sill[:offset]]]]`
fn placement(spec: &str, glow: f32) -> Result<Placement, String> {
    let mut parts = spec.split(':');
    let path = parts.next().unwrap_or_default().to_string();
    let wall = parts.next().map_or(Ok(Wall::Back), |w| {
        Wall::parse(w).ok_or_else(|| format!("{spec}: wall is back, front, left or right"))
    })?;
    let num = |v: Option<&str>, default: f32| -> Result<f32, String> {
        v.map_or(Ok(default), |v| v.parse().map_err(|_| format!("{spec}: {v} is no number")))
    };
    let height = num(parts.next(), if glow > 0.0 { 6.0 } else { 2.0 })?;
    let sill = num(parts.next(), if glow > 0.0 { 1.5 } else { 0.0 })?;
    let offset = num(parts.next(), 0.0)?;
    Ok(Placement {
        path,
        wall,
        height,
        sill,
        offset,
        glow,
    })
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fframes-walk: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let glow: f32 = value("--glow").and_then(|v| v.parse().ok()).unwrap_or(1.6);
    let mut placements = Vec::new();
    for (i, a) in args.iter().enumerate() {
        let g = match a.as_str() {
            "--window" => glow,
            "--model" => 0.0,
            _ => continue,
        };
        let spec = args.get(i + 1).ok_or_else(|| format!("{a} needs a file\n{}", usage()))?;
        placements.push(placement(spec, g)?);
    }
    if placements.is_empty() {
        return Err(usage().into());
    }
    let size = match value("--room") {
        Some(r) => {
            let v: Vec<f32> = r.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() != 3 {
                return Err("--room is width,depth,height in metres".into());
            }
            RoomSize {
                width: v[0],
                depth: v[1],
                height: v[2],
            }
        }
        None => {
            // wide and high enough for the largest model, a nave twice as deep as wide
            let tallest = placements.iter().map(|p| p.sill + p.height).fold(0.0, f32::max);
            let height = (tallest + 2.5).max(5.0);
            RoomSize {
                width: (height * 1.4).max(8.0),
                depth: (height * 2.8).max(16.0),
                height,
            }
        }
    };
    let quality: f32 = value("--quality")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.5_f32)
        .clamp(0.1, 1.0);

    let t = Instant::now();
    let mut room = room::build(size, &placements);
    // --at x,z: where to start (metres from the middle of the floor)
    if let Some(at) = value("--at") {
        let v: Vec<f32> = at.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if let [x, z] = v[..] {
            room.start = vec3(x, room.start.y, z);
        }
    }
    eprintln!(
        "room {:.1} x {:.1} x {:.1} m, built in {:.1} s",
        size.width,
        size.depth,
        size.height,
        t.elapsed().as_secs_f32()
    );

    if let Some(path) = value("--video") {
        let seconds: f32 = value("--seconds").and_then(|v| v.parse().ok()).unwrap_or(15.0);
        let (w, h) = value("--size")
            .and_then(|v| {
                let (a, b) = v.split_once('x')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .unwrap_or((1920_u32, 1080_u32));
        let captions = match value("--captions") {
            Some(c) => read_captions(&c)?,
            None => Vec::new(),
        };
        let demo = args.iter().any(|a| a == "--demo");
        return video(&room, &path, seconds, w, h, &captions, demo);
    }

    if let Some(path) = value("--snapshot") {
        let camera = Camera::perspective(70.0)
            .position(room.start)
            .look_at(room.start + vec3(0.0, 0.0, -1.0))
            .clip(0.05, 300.0);
        let renderer = Renderer::new(1280, 800).gpu().bloom(Bloom {
            strength: 0.5,
            radius: 0.6,
            threshold: 1.0,
        });
        let pixels = renderer.render_pixels(&room.scene, &camera);
        let info = ImageInfo::new(
            (pixels.width as i32, pixels.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let png = skia_safe::images::raster_from_data(
            &info,
            Data::new_copy(&pixels.rgba),
            pixels.width as usize * 4,
        )
        .and_then(|i| i.encode(None, skia_safe::EncodedImageFormat::PNG, None))
        .ok_or("could not encode the picture")?;
        std::fs::write(&path, png.as_bytes()).map_err(|e| format!("{path}: {e}"))?;
        eprintln!(
            "{path} ({})",
            if renderer.uses_gpu() { "GPU" } else { "CPU" }
        );
        return Ok(());
    }

    let font_mgr = FontMgr::new();
    let typeface = SYSTEM_FONTS
        .iter()
        .find_map(|f| font_mgr.match_family_style(f, FontStyle::normal()))
        .or_else(|| font_mgr.legacy_make_typeface(None, FontStyle::normal()))
        .ok_or("no system font available")?;
    let mut walk = Walk {
        position: room.start,
        yaw: room.start_yaw,
        pitch: 0.0,
        room,
        quality,
        renderer: None,
        window: None,
        surface: None,
        typeface,
        keys: HashSet::new(),
        last_step: Instant::now(),
        cursor: PhysicalPosition::default(),
        dragging: None,
        fps: 0.0,
        error: None,
    };
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.run_app(&mut walk).map_err(|e| e.to_string())?;
    walk.error.map_or(Ok(()), Err)
}

/// Smooth start and stop.
fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The camera of the walk at `u` in `0..1`: from the start to a few metres before the first
/// window (or the back wall), looking at it, at the end tilting up to its upper part.
fn tour_camera(room: &room::Room, u: f32) -> Camera {
    let (centre, into_room, height) = room.windows.first().copied().unwrap_or((
        vec3(0.0, room.size.height / 2.0, -room.size.depth / 2.0),
        vec3(0.0, 0.0, 1.0),
        room.size.height / 2.0,
    ));
    let end = centre + into_room * (height * 0.65).max(2.5);
    let end = vec3(end.x, room.start.y, end.z);
    let walk = ease(u / ZOOM_FROM);
    let position = room.start + (end - room.start) * walk;
    // look at the window's middle, then let the gaze rise to its upper third
    let look = centre + vec3(0.0, height * 0.18 * ease((u - 0.45) / (ZOOM_FROM - 0.45)), 0.0);
    // a slight sway of the steps, fading out when standing
    let sway = (u * 9.0 * std::f32::consts::TAU).sin() * 0.012 * (1.0 - walk);
    let position = position + vec3(0.0, sway, 0.0);
    if u > ZOOM_FROM {
        return zoom_end(room, position, look, u);
    }
    Camera::perspective(62.0).position(position).look_at(look).clip(0.05, 300.0)
}

/// What is held in a moment of the demo: keys and the mouse.
#[derive(Clone, Copy, Default)]
struct Held {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    shift: bool,
    mouse: bool,
}

/// The demo script, as fractions of its length: (until, held, look per second (yaw, pitch)).
/// The same speeds as the window: 1.6 m/s walking, 4.5 m/s with Shift.
const DEMO: [(f32, Held, (f32, f32)); 10] = [
    (0.05, Held { w: false, a: false, s: false, d: false, shift: false, mouse: false }, (0.0, 0.0)),
    (0.21, Held { w: true, a: false, s: false, d: false, shift: false, mouse: false }, (0.0, 0.0)),
    (0.29, Held { w: false, a: true, s: false, d: false, shift: false, mouse: false }, (0.0, 0.0)),
    (0.35, Held { w: false, a: false, s: false, d: false, shift: false, mouse: true }, (0.55, 0.12)),
    (0.41, Held { w: false, a: false, s: false, d: false, shift: false, mouse: true }, (-0.55, -0.12)),
    (0.49, Held { w: false, a: false, s: false, d: true, shift: false, mouse: false }, (0.0, 0.0)),
    (0.59, Held { w: true, a: false, s: false, d: false, shift: true, mouse: false }, (0.0, 0.0)),
    (0.65, Held { w: false, a: false, s: true, d: false, shift: false, mouse: false }, (0.0, 0.0)),
    (0.78, Held { w: true, a: false, s: false, d: false, shift: false, mouse: true }, (0.0, 0.1)),
    // the lens zoom of `zoom_end`: nothing held
    (1.0, Held { w: false, a: false, s: false, d: false, shift: false, mouse: false }, (0.0, 0.0)),
];

/// The camera and the held keys at `time` of a demo `seconds` long, by playing the script
/// from the start with the movement of the window.
fn demo_state(room: &room::Room, time: f32, seconds: f32) -> (Camera, Held) {
    let dt = 1.0 / 120.0;
    let (mut p, mut yaw, mut pitch) = (room.start, room.start_yaw, 0.0_f32);
    let mut held = Held::default();
    let mut t = 0.0;
    while t <= time {
        let u = t / seconds;
        let (_, h, (dyaw, dpitch)) = DEMO.iter().find(|(until, ..)| u <= *until).copied().unwrap_or(DEMO[DEMO.len() - 1]);
        held = h;
        let speed = if h.shift { 4.5 } else { 1.6 };
        let ahead = vec3(-yaw.sin(), 0.0, -yaw.cos());
        let right = vec3(yaw.cos(), 0.0, -yaw.sin());
        let mut v = Vec3::ZERO;
        if h.w {
            v += ahead;
        }
        if h.s {
            v -= ahead;
        }
        if h.d {
            v += right;
        }
        if h.a {
            v -= right;
        }
        if v.length_squared() > 0.0 {
            p += v.normalize() * speed * dt;
        }
        yaw += dyaw * dt;
        pitch = (pitch + dpitch * dt).clamp(-1.4, 1.4);
        let s = room.size;
        p.x = p.x.clamp(-s.width / 2.0 + 0.35, s.width / 2.0 - 0.35);
        p.z = p.z.clamp(-s.depth / 2.0 + 0.35, s.depth / 2.0 - 0.35);
        t += dt;
    }
    let forward = vec3(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    let u = time / seconds;
    let camera = if u > ZOOM_FROM {
        zoom_end(room, p, p + forward, u)
    } else {
        Camera::perspective(62.0)
            .position(p)
            .look_at(p + forward)
            .clip(0.05, 300.0)
    };
    (camera, held)
}

/// The keys W A S D, Shift and a mouse, top right; held ones lit.
fn draw_keys(rgba: &mut [u8], w: u32, h: u32, held: Held, typeface: &skia_safe::Typeface) {
    let info = ImageInfo::new((w as i32, h as i32), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(canvas) = skia_safe::Canvas::from_raster_direct(&info, rgba, w as usize * 4, None)
    else {
        return;
    };
    let u = h as f32 / 1080.0;
    let k = 64.0 * u;
    let gap = 10.0 * u;
    let right = w as f32 - 72.0 * u;
    let top = 64.0 * u;
    let mut font = Font::from_typeface(typeface, 28.0 * u);
    font.set_edging(skia_safe::font::Edging::AntiAlias);
    let key = |canvas: &skia_safe::Canvas, x: f32, y: f32, width: f32, label: &str, on: bool| {
        let r = skia_safe::RRect::new_rect_xy(Rect::from_xywh(x, y, width, k), 10.0 * u, 10.0 * u);
        let mut fill = Paint::default();
        fill.set_anti_alias(true);
        fill.set_color(if on {
            Color::from_argb(235, 255, 214, 120)
        } else {
            Color::from_argb(120, 20, 20, 24)
        });
        canvas.draw_rrect(r, &fill);
        let mut edge = Paint::default();
        edge.set_anti_alias(true);
        edge.set_style(skia_safe::PaintStyle::Stroke);
        edge.set_stroke_width(2.0 * u);
        edge.set_color(Color::from_argb(200, 240, 235, 225));
        canvas.draw_rrect(r, &edge);
        let mut text = Paint::default();
        text.set_anti_alias(true);
        text.set_color(if on { Color::from_rgb(30, 25, 15) } else { Color::from_rgb(240, 235, 225) });
        let (tw, _) = font.measure_str(label, Some(&text));
        canvas.draw_str(label, (x + (width - tw) / 2.0, y + k * 0.68), &font, &text);
    };
    // mouse, then D S A in a row, W above S, Shift left of A
    let mouse_w = 48.0 * u;
    let mx = right - mouse_w;
    let row_y = top + k + gap;
    let d_x = mx - 28.0 * u - k;
    let s_x = d_x - gap - k;
    let a_x = s_x - gap - k;
    let shift_w = 1.7 * k;
    key(&canvas, s_x, top, k, "W", held.w);
    key(&canvas, a_x, row_y, k, "A", held.a);
    key(&canvas, s_x, row_y, k, "S", held.s);
    key(&canvas, d_x, row_y, k, "D", held.d);
    key(&canvas, a_x - gap - shift_w, row_y, shift_w, "Shift", held.shift);
    // the mouse: a rounded body with a line between the buttons
    let body = skia_safe::RRect::new_rect_xy(
        Rect::from_xywh(mx, top + 6.0 * u, mouse_w, 2.0 * k),
        mouse_w / 2.0,
        mouse_w / 2.0,
    );
    let mut fill = Paint::default();
    fill.set_anti_alias(true);
    fill.set_color(if held.mouse {
        Color::from_argb(235, 255, 214, 120)
    } else {
        Color::from_argb(120, 20, 20, 24)
    });
    canvas.draw_rrect(body, &fill);
    let mut edge = Paint::default();
    edge.set_anti_alias(true);
    edge.set_style(skia_safe::PaintStyle::Stroke);
    edge.set_stroke_width(2.0 * u);
    edge.set_color(Color::from_argb(200, 240, 235, 225));
    canvas.draw_rrect(body, &edge);
    canvas.draw_line(
        (mx + mouse_w / 2.0, top + 6.0 * u),
        (mx + mouse_w / 2.0, top + 6.0 * u + 0.8 * k),
        &edge,
    );
    let mut label = Paint::default();
    label.set_anti_alias(true);
    label.set_color(Color::from_argb(220, 240, 235, 225));
    let mut small = Font::from_typeface(typeface, 20.0 * u);
    small.set_edging(skia_safe::font::Edging::AntiAlias);
    let (tw, _) = small.measure_str("look", Some(&label));
    canvas.draw_str("look", (mx + (mouse_w - tw) / 2.0, top + 2.0 * k + 34.0 * u), &small, &label);
    drop(canvas);
}

/// A caption: from, to (seconds) and its lines.
type Caption = (f32, f32, Vec<String>);

fn read_captions(path: &str) -> Result<Vec<Caption>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let mut parts = line.split('|');
        let span = parts.next().unwrap_or_default();
        let (a, b) = span
            .split_once('-')
            .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
            .ok_or_else(|| format!("{path}: \"{span}\" is not start-end"))?;
        out.push((a, b, parts.map(str::to_string).collect()));
    }
    Ok(out)
}

/// Draws the captions shown at `time` onto premultiplied RGBA pixels, fading in and out.
fn draw_captions(
    rgba: &mut [u8],
    w: u32,
    h: u32,
    captions: &[Caption],
    time: f32,
    typeface: &skia_safe::Typeface,
) {
    let info = ImageInfo::new((w as i32, h as i32), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(canvas) = skia_safe::Canvas::from_raster_direct(&info, rgba, w as usize * 4, None)
    else {
        return;
    };
    let unit = h as f32 / 1080.0;
    for (a, b, lines) in captions.iter().filter(|(a, b, _)| time >= *a && time <= *b) {
        let fade = ((time - a) / 0.5).min((b - time) / 0.5).clamp(0.0, 1.0);
        let alpha = |x: f32| (x * fade * 255.0) as u8;
        let x = 96.0 * unit;
        let mut y = h as f32 - 96.0 * unit - (lines.len().saturating_sub(1)) as f32 * 46.0 * unit;
        let mut band = Paint::default();
        band.set_color(Color::from_argb(alpha(0.55), 0, 0, 0));
        let top = y - 62.0 * unit;
        canvas.draw_rect(Rect::from_ltrb(0.0, top, w as f32, h as f32), &band);
        for (i, line) in lines.iter().enumerate() {
            let (size, colour) = if i == 0 {
                (52.0, Color::from_argb(alpha(1.0), 255, 248, 235))
            } else {
                (34.0, Color::from_argb(alpha(0.9), 225, 215, 195))
            };
            let mut font = Font::from_typeface(typeface, size * unit);
            font.set_subpixel(true);
            font.set_edging(skia_safe::font::Edging::AntiAlias);
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color(colour);
            canvas.draw_str(line, (x, y), &font, &paint);
            y += if i == 0 { 58.0 } else { 46.0 } * unit;
        }
    }
    drop(canvas);
}

/// The last part of a video zooms in with the lens (the visitor stays at eye height): the
/// gaze settles on a point of the first window and the field of view narrows to `ZOOM_FOV`.
const ZOOM_FROM: f32 = 0.78;
const ZOOM_FOV: f32 = 7.0;

/// `camera` turned towards a detail of the first window and zoomed in, by `u` in `0..1`.
fn zoom_end(room: &room::Room, position: Vec3, look: Vec3, u: f32) -> Camera {
    let e = ease((u - ZOOM_FROM) / (1.0 - ZOOM_FROM));
    let (centre, _, height) = room.windows.first().copied().unwrap_or((
        vec3(0.0, room.size.height / 2.0, -room.size.depth / 2.0),
        vec3(0.0, 0.0, 1.0),
        room.size.height / 2.0,
    ));
    let detail = centre + vec3(0.0, height * 0.12, 0.0);
    // aim along the current gaze at the window's distance, then onto the detail
    let dist = (detail - position).length();
    let gaze = position + (look - position).normalize_or_zero() * dist;
    let target = gaze + (detail - gaze) * e;
    let fov = 62.0 + (ZOOM_FOV - 62.0) * e;
    Camera::perspective(fov)
        .position(position)
        .look_at(target)
        .clip(0.05, 300.0)
}

/// Renders the walk and pipes the frames into `ffmpeg`.
fn video(
    room: &room::Room,
    path: &str,
    seconds: f32,
    w: u32,
    h: u32,
    captions: &[Caption],
    demo: bool,
) -> Result<(), String> {
    let font_mgr = FontMgr::new();
    let typeface = SYSTEM_FONTS
        .iter()
        .find_map(|f| font_mgr.match_family_style(f, FontStyle::normal()))
        .or_else(|| font_mgr.legacy_make_typeface(None, FontStyle::normal()))
        .ok_or("no system font available")?;
    use std::io::Write;
    let fps = 30;
    let frames = (seconds * fps as f32).round().max(1.0) as u32;
    let mut ffmpeg = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
        .arg(format!("{w}x{h}"))
        .args(["-r", &fps.to_string(), "-i", "-", "-c:v", "libx264", "-preset", "slow"])
        .args(["-crf", "17", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    let mut stdin = ffmpeg.stdin.take().ok_or("ffmpeg: no input")?;
    let renderer = Renderer::new(w, h).gpu().bloom(Bloom {
        strength: 0.5,
        radius: 0.6,
        threshold: 1.0,
    });
    let t = Instant::now();
    for f in 0..frames {
        let u = f as f32 / (frames - 1).max(1) as f32;
        let time = f as f32 / fps as f32;
        let (camera, held) = if demo {
            let (camera, held) = demo_state(room, time, seconds);
            (camera, Some(held))
        } else {
            (tour_camera(room, u), None)
        };
        let mut pixels = renderer.render_pixels(&room.scene, &camera);
        draw_captions(&mut pixels.rgba, w, h, captions, time, &typeface);
        if let Some(held) = held {
            draw_keys(&mut pixels.rgba, w, h, held, &typeface);
        }
        stdin.write_all(&pixels.rgba).map_err(|e| format!("ffmpeg: {e}"))?;
        if f % fps == 0 {
            eprint!("\r{}/{frames} frames", f + 1);
        }
    }
    drop(stdin);
    let status = ffmpeg.wait().map_err(|e| format!("ffmpeg: {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg failed: {status}"));
    }
    eprintln!(
        "\r{path}: {frames} frames, {seconds} s, {w}x{h}, rendered in {:.0} s ({})",
        t.elapsed().as_secs_f32(),
        if renderer.uses_gpu() { "GPU" } else { "CPU" }
    );
    Ok(())
}

struct Walk {
    room: room::Room,
    position: Vec3,
    /// Radians, 0 = looking at the back wall (-z).
    yaw: f32,
    pitch: f32,
    quality: f32,
    renderer: Option<(u32, u32, Renderer)>,
    window: Option<Rc<Window>>,
    surface: Option<Surface>,
    typeface: skia_safe::Typeface,
    keys: HashSet<KeyCode>,
    last_step: Instant,
    cursor: PhysicalPosition<f64>,
    dragging: Option<PhysicalPosition<f64>>,
    fps: f32,
    error: Option<String>,
}

impl Walk {
    fn forward(&self) -> Vec3 {
        vec3(
            -self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    fn camera(&self) -> Camera {
        Camera::perspective(70.0)
            .position(self.position)
            .look_at(self.position + self.forward())
            .clip(0.05, 300.0)
    }

    fn moving(&self) -> bool {
        [
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
        ]
        .iter()
        .any(|k| self.keys.contains(k))
    }

    /// Moves by the held keys for the time since the last step, inside the walls.
    fn step(&mut self) {
        let dt = self.last_step.elapsed().as_secs_f32().min(0.1);
        self.last_step = Instant::now();
        let held = |k: KeyCode| self.keys.contains(&k);
        let speed = if held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight) { 4.5 } else { 1.6 };
        let ahead = vec3(-self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = vec3(self.yaw.cos(), 0.0, -self.yaw.sin());
        let mut v = Vec3::ZERO;
        if held(KeyCode::KeyW) || held(KeyCode::ArrowUp) {
            v += ahead;
        }
        if held(KeyCode::KeyS) || held(KeyCode::ArrowDown) {
            v -= ahead;
        }
        if held(KeyCode::KeyD) {
            v += right;
        }
        if held(KeyCode::KeyA) {
            v -= right;
        }
        if held(KeyCode::ArrowLeft) {
            self.yaw += 1.5 * dt;
        }
        if held(KeyCode::ArrowRight) {
            self.yaw -= 1.5 * dt;
        }
        if v.length_squared() > 0.0 {
            self.position += v.normalize() * speed * dt;
        }
        let s = self.room.size;
        let margin = 0.35;
        self.position.x = self.position.x.clamp(-s.width / 2.0 + margin, s.width / 2.0 - margin);
        self.position.z = self.position.z.clamp(-s.depth / 2.0 + margin, s.depth / 2.0 - margin);
    }

    fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.004;
        self.pitch = (self.pitch - dy * 0.004).clamp(-1.4, 1.4);
    }

    fn draw(&mut self) {
        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        let (rw, rh) = (
            ((size.width as f32 * self.quality) as u32).max(16),
            ((size.height as f32 * self.quality) as u32).max(16),
        );
        if self.renderer.as_ref().is_none_or(|(w, h, _)| (*w, *h) != (rw, rh)) {
            let r = Renderer::new(rw, rh).gpu().bloom(Bloom {
                strength: 0.5,
                radius: 0.6,
                threshold: 1.0,
            });
            if !r.uses_gpu() {
                eprintln!("no GPU found, rendering on the CPU (slow)");
            }
            self.renderer = Some((rw, rh, r));
        }
        let t = Instant::now();
        let camera = self.camera();
        let Some((_, _, renderer)) = self.renderer.as_ref() else { return };
        let pixels = renderer.render_pixels(&self.room.scene, &camera);
        let info = ImageInfo::new(
            (pixels.width as i32, pixels.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let image = skia_safe::images::raster_from_data(
            &info,
            Data::new_copy(&pixels.rgba),
            pixels.width as usize * 4,
        );
        let ui = window.scale_factor() as f32;
        let font = Font::from_typeface(&self.typeface, 13.0 * ui);
        let fps = self.fps;
        let p = self.position;
        let status = format!(
            "{fps:.0} fps   x {:.1}  z {:.1}   W A S D walk, Shift faster, drag to look, R start, Q quit",
            p.x, p.z
        );
        let Some(surface) = self.surface.as_mut() else { return };
        let result = surface.present(&mut |canvas, _| {
            canvas.clear(Color::BLACK);
            let size = canvas.base_layer_size();
            let (w, h) = (size.width as f32, size.height as f32);
            if let Some(image) = &image {
                canvas.draw_image_rect_with_sampling_options(
                    image,
                    None,
                    Rect::from_wh(w, h),
                    SamplingOptions::new(skia_safe::FilterMode::Linear, skia_safe::MipmapMode::None),
                    &Paint::default(),
                );
            }
            let mut text = Paint::default();
            text.set_anti_alias(true);
            text.set_color(Color::from_argb(160, 0, 0, 0));
            canvas.draw_rect(Rect::from_xywh(0.0, h - 26.0 * ui, w, 26.0 * ui), &text);
            text.set_color(Color::from_rgb(230, 230, 230));
            canvas.draw_str(&status, (10.0 * ui, h - 9.0 * ui), &font, &text);
        });
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        let ms = t.elapsed().as_secs_f32();
        self.fps = 0.8 * self.fps + 0.2 / ms.max(0.001);
    }
}

impl ApplicationHandler for Walk {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("fframes-walk")
            .with_inner_size(LogicalSize::new(1280, 800));
        let window = match event_loop.create_window(attributes) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                self.error = Some(e.to_string());
                return event_loop.exit();
            }
        };
        match Surface::new(window.clone(), PlayerBackend::Auto, true) {
            Ok(s) => self.surface = Some(s),
            Err(e) => {
                self.error = Some(e.to_string());
                return event_loop.exit();
            }
        }
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(Err(e)) = self.surface.as_mut().map(Surface::resize) {
                    self.error = Some(e.to_string());
                    return event_loop.exit();
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.step();
                self.draw();
                if self.error.is_some() {
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            match code {
                                KeyCode::KeyQ | KeyCode::Escape => event_loop.exit(),
                                KeyCode::KeyR => {
                                    self.position = self.room.start;
                                    self.yaw = self.room.start_yaw;
                                    self.pitch = 0.0;
                                }
                                _ => {}
                            }
                            if self.keys.insert(code) {
                                self.last_step = Instant::now();
                            }
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                        }
                    }
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(from) = self.dragging {
                    self.look((position.x - from.x) as f32, (position.y - from.y) as f32);
                    self.dragging = Some(position);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                self.cursor = position;
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left | MouseButton::Right, .. } => {
                self.dragging = (state == ElementState::Pressed).then_some(self.cursor);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                self.look(-dx, -dy);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.moving() {
            event_loop.set_control_flow(ControlFlow::Poll);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

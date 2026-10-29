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

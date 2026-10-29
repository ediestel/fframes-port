//! The editor window: the pieces drawn with Skia into a [`Surface`] of the native player,
//! the zones over them, grouping and the report redone after every change.

use std::path::PathBuf;
use std::rc::Rc;

use fframes_native_player::fframes_skia_renderer::skia_safe::{
    self, Canvas, Color, Font, FontMgr, FontStyle, Matrix, Paint, PaintStyle, Path, Rect,
};
use fframes_native_player::winit;
use fframes_native_player::{PlayerBackend, Surface};
use fframes_vector::cut::{self, CutParams};
use fframes_vector::group::{self, AssignParams, GroupMap, Zone};
use fframes_vector::report::{self, Doubt, Report, ReportParams};
use fframes_vector::{PieceDocument, PieceRaster};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

pub struct Config {
    pub doc: PathBuf,
    pub zones: Option<PathBuf>,
    pub out: PathBuf,
    pub zones_out: PathBuf,
    /// Zone coordinates to document pixels; the document's scale by default.
    pub scale: Option<f32>,
    pub cut: bool,
}

/// Up to this many pieces the cut is redone after every change; above it, only on saving.
const LIVE_CUT_MAX_PIECES: usize = 3000;

const SYSTEM_FONTS: &[&str] = &[".AppleSystemUIFont", "Helvetica Neue", "Segoe UI", "DejaVu Sans"];

fn hex(c: &str) -> Color {
    let v = u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0);
    Color::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The document, its pieces as Skia paths, the zones and what grouping made of them.
struct Model {
    /// The document as loaded.
    base: PieceDocument,
    /// What is shown and saved: the base, or the base cut along the zones.
    doc: PieceDocument,
    /// Cut along the zones after every change (small documents; large ones cut on save).
    live_cut: bool,
    paths: Vec<Option<Path>>,
    fills: Vec<Color>,
    raster: PieceRaster,
    zones: GroupMap,
    zone_scale: f32,
    /// Zones in document pixels, in zone order.
    zone_paths: Vec<Option<Path>>,
    groups: Vec<Option<Color>>,
    report: Report,
}

impl Model {
    fn new(doc: PieceDocument, zones: GroupMap, zone_scale: f32, live_cut: bool) -> Self {
        let paths = doc.pieces.iter().map(|p| Path::from_svg(&p.d)).collect();
        let fills = doc.pieces.iter().map(|p| hex(doc.color_hex(p))).collect();
        let raster = PieceRaster::fit(&doc, 4096);
        let mut m = Model {
            groups: vec![None; doc.pieces.len()],
            base: doc.clone(),
            doc,
            live_cut,
            paths,
            fills,
            raster,
            zones,
            zone_scale,
            zone_paths: Vec::new(),
            report: Report::default(),
        };
        m.regroup();
        m
    }

    /// Shows `doc` instead of the current document.
    fn show(&mut self, doc: PieceDocument) {
        self.paths = doc.pieces.iter().map(|p| Path::from_svg(&p.d)).collect();
        self.fills = doc.pieces.iter().map(|p| hex(doc.color_hex(p))).collect();
        self.raster = PieceRaster::fit(&doc, 4096);
        self.doc = doc;
    }

    /// Grouping and the report after a change of zones (cutting first when live).
    fn regroup(&mut self) {
        let mut named = Vec::new();
        if self.live_cut {
            let mut doc = self.base.clone();
            named = cut::cut_at_zones(&mut doc, &self.zones, self.zone_scale, &CutParams::default())
                .named;
            self.show(doc);
        } else if self.doc.pieces.len() != self.base.pieces.len() {
            self.show(self.base.clone());
        }
        let k = Matrix::scale((self.zone_scale, self.zone_scale));
        self.zone_paths = self
            .zones
            .zones
            .iter()
            .map(|z| Path::from_svg(z.path_data()).map(|p| p.with_transform(&k)))
            .collect();
        let layer = self.zones.layer(&self.raster, self.zone_scale);
        group::assign(&mut self.doc, &self.raster, &layer, &AssignParams::default());
        cut::keep_names(&mut self.doc, &named);
        self.report = report::report(&self.doc, &self.raster, &layer, &ReportParams::default());
        self.groups = self
            .doc
            .pieces
            .iter()
            .map(|p| (!p.group.is_empty()).then(|| hex(&group::group_color(&p.group))))
            .collect();
    }

    /// The visible piece at a document point.
    fn piece_at(&self, x: f32, y: f32) -> Option<usize> {
        let r = &self.raster;
        let (px, py) = ((x * r.res) as i64, (y * r.res) as i64);
        if px < 0 || py < 0 || px >= i64::from(r.width) || py >= i64::from(r.height) {
            return None;
        }
        let l = r.labels[py as usize * r.width as usize + px as usize];
        (l > 0).then(|| l as usize - 1)
    }

    /// The zone on top (earliest in the file) at a document point.
    fn zone_at(&self, x: f32, y: f32) -> Option<usize> {
        self.zone_paths
            .iter()
            .position(|p| p.as_ref().is_some_and(|p| p.contains((x, y))))
    }

    fn grouped(&self) -> usize {
        self.doc.pieces.iter().filter(|p| !p.group.is_empty()).count()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Picture,
    Mix,
    Groups,
}

enum Tool {
    Idle,
    /// Points of a new zone, in document pixels.
    Drawing(Vec<(f32, f32)>),
    /// The closed zone waiting for its name.
    Naming(Vec<(f32, f32)>, String),
    /// The selected pieces waiting for their name.
    NamingSelection(String),
}

struct View {
    scale: f32,
    x: f32,
    y: f32,
}

impl View {
    fn to_doc(&self, sx: f32, sy: f32) -> (f32, f32) {
        ((sx - self.x) / self.scale, (sy - self.y) / self.scale)
    }

    fn zoom_at(&mut self, factor: f32, sx: f32, sy: f32) {
        let s = (self.scale * factor).clamp(0.01, 200.0);
        let f = s / self.scale;
        self.x = sx - (sx - self.x) * f;
        self.y = sy - (sy - self.y) * f;
        self.scale = s;
    }
}

struct Editor {
    config: Config,
    model: Model,
    window: Option<Rc<Window>>,
    surface: Option<Surface>,
    font_regular: skia_safe::Typeface,
    error: Option<String>,

    view: View,
    fitted: bool,
    look: Look,
    show_doubts: bool,
    tool: Tool,
    last_name: String,
    message: String,
    doubt_index: Option<usize>,

    cursor: PhysicalPosition<f64>,
    hovered: Option<usize>,
    modifiers: ModifiersState,
    dragging: Option<PhysicalPosition<f64>>,

    /// Zones before each change, for undo; and the undone ones, for redo.
    undo: Vec<GroupMap>,
    redo: Vec<GroupMap>,

    /// Selected pieces (indices), named together with `Enter`.
    selected: Vec<usize>,
    /// Where the left button went down, to tell a click from a drag.
    pressed_at: Option<PhysicalPosition<f64>>,
}

pub fn run(config: Config) -> Result<(), String> {
    let doc = PieceDocument::load(&config.doc)
        .map_err(|e| format!("{}: {e}", config.doc.display()))?;
    let zones = match &config.zones {
        Some(z) => GroupMap::from_svg(
            &std::fs::read_to_string(z).map_err(|e| format!("{}: {e}", z.display()))?,
        ),
        None => GroupMap::default(),
    };
    let zone_scale = config.scale.unwrap_or(doc.scale as f32);
    eprintln!(
        "{}: {} pieces, {} zones",
        config.doc.display(),
        doc.pieces.len(),
        zones.zones.len()
    );
    let small = doc.pieces.len() <= LIVE_CUT_MAX_PIECES;
    let font_mgr = FontMgr::new();
    let font_regular = SYSTEM_FONTS
        .iter()
        .find_map(|f| font_mgr.match_family_style(f, FontStyle::normal()))
        .or_else(|| font_mgr.legacy_make_typeface(None, FontStyle::normal()))
        .ok_or("no system font available")?;
    let mut editor = Editor {
        model: Model::new(doc, zones, zone_scale, config.cut && small),
        config,
        window: None,
        surface: None,
        font_regular,
        error: None,
        view: View {
            scale: 1.0,
            x: 0.0,
            y: 0.0,
        },
        fitted: false,
        look: Look::Mix,
        show_doubts: true,
        tool: Tool::Idle,
        last_name: String::new(),
        message: "z: new zone   s: save   ?: keys in the terminal".into(),
        doubt_index: None,
        cursor: PhysicalPosition::default(),
        hovered: None,
        modifiers: ModifiersState::default(),
        dragging: None,
        undo: Vec::new(),
        redo: Vec::new(),
        selected: Vec::new(),
        pressed_at: None,
    };
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.run_app(&mut editor).map_err(|e| e.to_string())?;
    match editor.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

impl Editor {
    fn redraw_soon(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_size(&self) -> (f32, f32) {
        self.window.as_ref().map_or((1.0, 1.0), |w| {
            let s = w.inner_size();
            (s.width as f32, s.height as f32)
        })
    }

    fn fit(&mut self) {
        let (w, h) = self.window_size();
        let h = h - self.bar_height();
        let (dw, dh) = (self.model.doc.width as f32, self.model.doc.height as f32);
        let s = (w / dw).min(h / dh) * 0.96;
        self.view = View {
            scale: s,
            x: (w - dw * s) / 2.0,
            y: (h - dh * s) / 2.0,
        };
    }

    fn ui_scale(&self) -> f32 {
        self.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32)
    }

    fn bar_height(&self) -> f32 {
        44.0 * self.ui_scale()
    }

    fn cursor_doc(&self) -> (f32, f32) {
        self.view.to_doc(self.cursor.x as f32, self.cursor.y as f32)
    }

    fn jump_to_doubt(&mut self, step: isize) {
        let n = self.model.report.entries.len();
        if n == 0 {
            self.message = "no doubtful pieces".into();
            return;
        }
        let i = match self.doubt_index {
            None if step < 0 => n - 1,
            None => 0,
            Some(i) => (i as isize + step).rem_euclid(n as isize) as usize,
        };
        self.doubt_index = Some(i);
        let e = &self.model.report.entries[i];
        let [x, y, w, h] = e.bbox;
        let (ww, wh) = self.window_size();
        let wh = wh - self.bar_height();
        let s = ((ww.min(wh) * 0.4) / w.max(h).max(1.0)).clamp(0.01, 200.0);
        self.view = View {
            scale: s,
            x: ww / 2.0 - (x + w / 2.0) * s,
            y: wh / 2.0 - (y + h / 2.0) * s,
        };
        let shares: Vec<String> = e
            .shares
            .iter()
            .take(3)
            .map(|(n, s)| format!("{} {:.0} %", if n.is_empty() { "-" } else { n }, s * 100.0))
            .collect();
        self.message = format!(
            "doubtful {}/{n}: piece {} ({:?}), {}",
            i + 1,
            e.id,
            e.doubt,
            shares.join(", ")
        );
    }

    fn save(&mut self) {
        let src_w = (self.model.doc.width as f32 / self.model.zone_scale).round() as u32;
        let src_h = (self.model.doc.height as f32 / self.model.zone_scale).round() as u32;
        let zones_svg = self.model.zones.to_svg(src_w, src_h);
        if let Err(e) = std::fs::write(&self.config.zones_out, zones_svg) {
            self.message = format!("{}: {e}", self.config.zones_out.display());
            return;
        }
        let mut doc = self.model.doc.clone();
        let mut cut_note = String::new();
        if self.model.live_cut {
            cut_note = format!(
                ", cut: {} pieces (from {})",
                doc.pieces.len(),
                self.model.base.pieces.len()
            );
        } else if self.config.cut {
            let s = cut::cut_at_zones(
                &mut doc,
                &self.model.zones,
                self.model.zone_scale,
                &CutParams::default(),
            );
            let raster = PieceRaster::fit(&doc, 4096);
            let layer = self.model.zones.layer(&raster, self.model.zone_scale);
            group::assign(&mut doc, &raster, &layer, &AssignParams::default());
            cut::keep_names(&mut doc, &s.named);
            cut_note = format!(", {} pieces cut into {} parts", s.cut, s.parts);
        }
        match doc.save(&self.config.out) {
            Ok(()) => {
                self.message = format!(
                    "saved {} and {}{cut_note}",
                    self.config.zones_out.display(),
                    self.config.out.display()
                );
            }
            Err(e) => self.message = format!("{}: {e}", self.config.out.display()),
        }
        eprintln!("{}", self.message);
    }

    /// Undo (`back`) or redo the last change to the zones.
    fn step_back(&mut self, back: bool) {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(zones) = from.pop() else {
            self.message = if back { "nothing to undo" } else { "nothing to redo" }.into();
            return;
        };
        to.push(std::mem::replace(&mut self.model.zones, zones));
        self.model.regroup();
                        if self.model.live_cut {
                            self.selected.clear();
                        }
        self.message = format!(
            "{}: {} zones, {} of {} pieces grouped ({} more steps back)",
            if back { "undone" } else { "redone" },
            self.model.zones.zones.len(),
            self.model.grouped(),
            self.model.doc.pieces.len(),
            self.undo.len()
        );
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        let key = event.logical_key.as_ref();
        let command = self.modifiers.super_key() || self.modifiers.control_key();
        match &mut self.tool {
            Tool::Naming(points, name) => match key {
                Key::Named(NamedKey::Enter) => {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        self.message = "a zone needs a name".into();
                    } else {
                        let k = self.model.zone_scale;
                        let src: Vec<(f32, f32)> = points.iter().map(|&(x, y)| (x / k, y / k)).collect();
                        if let Some(z) = Zone::polygon(&name, &src) {
                            // a new zone is usually more specific: it goes first and wins
                            self.undo.push(self.model.zones.clone());
                            self.redo.clear();
                            self.model.zones.zones.insert(0, z);
                            self.model.regroup();
                        if self.model.live_cut {
                            self.selected.clear();
                        }
                            self.message = format!(
                                "zone \"{name}\": {} of {} pieces grouped",
                                self.model.grouped(),
                                self.model.doc.pieces.len()
                            );
                        }
                        self.last_name = name;
                        self.tool = Tool::Idle;
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    self.tool = Tool::Idle;
                    self.message = "zone discarded".into();
                }
                Key::Named(NamedKey::Backspace) => {
                    name.pop();
                }
                _ => {
                    if let Some(text) = &event.text
                        && !self.modifiers.super_key()
                    {
                        name.extend(text.chars().filter(|c| !c.is_control()));
                    }
                }
            },
            Tool::NamingSelection(name) => match key {
                Key::Named(NamedKey::Enter) => {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        self.message = "the pieces need a name".into();
                    } else if let Some(z) = Zone::from_pieces(
                        &name,
                        &self.model.doc,
                        &self.selected,
                        self.model.zone_scale,
                    ) {
                        self.undo.push(self.model.zones.clone());
                        self.redo.clear();
                        // exactly these pieces: first, so it wins over rough zones
                        self.model.zones.zones.insert(0, z);
                        self.model.regroup();
                        if self.model.live_cut {
                            self.selected.clear();
                        }
                        self.message = format!(
                            "{} pieces named \"{name}\": {} of {} pieces grouped",
                            self.selected.len(),
                            self.model.grouped(),
                            self.model.doc.pieces.len()
                        );
                        self.selected.clear();
                        self.last_name = name;
                        self.tool = Tool::Idle;
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    self.tool = Tool::Idle;
                    self.message = "naming stopped, the pieces stay selected".into();
                }
                Key::Named(NamedKey::Backspace) => {
                    name.pop();
                }
                _ => {
                    if let Some(text) = &event.text
                        && !command
                    {
                        name.extend(text.chars().filter(|c| !c.is_control()));
                    }
                }
            },
            Tool::Drawing(points) => match key {
                Key::Character(c) if command && c.eq_ignore_ascii_case("z") => {
                    points.pop();
                }
                Key::Named(NamedKey::Enter) if points.len() >= 3 => {
                    let points = std::mem::take(points);
                    self.tool = Tool::Naming(points, self.last_name.clone());
                    self.message = "type the name (a/b for nested), Enter".into();
                }
                Key::Named(NamedKey::Backspace) => {
                    points.pop();
                }
                Key::Named(NamedKey::Escape) => {
                    self.tool = Tool::Idle;
                    self.message = String::new();
                }
                _ => {}
            },
            Tool::Idle => match key {
                Key::Character(c) if command && c.eq_ignore_ascii_case("z") => {
                    self.step_back(!self.modifiers.shift_key());
                }
                Key::Named(NamedKey::Enter) if !self.selected.is_empty() => {
                    self.tool = Tool::NamingSelection(self.last_name.clone());
                    self.message = format!(
                        "name for the {} selected pieces (a/b for nested), Enter",
                        self.selected.len()
                    );
                }
                Key::Named(NamedKey::Escape) if !self.selected.is_empty() => {
                    self.selected.clear();
                    self.message = "selection cleared".into();
                }
                Key::Character("u") => self.step_back(true),
                Key::Character("U") => self.step_back(false),
                Key::Character("q") | Key::Named(NamedKey::Escape) => event_loop.exit(),
                Key::Character("z") => {
                    self.tool = Tool::Drawing(Vec::new());
                    self.message = "click the corners of the zone, Enter to close".into();
                }
                Key::Character("x") => {
                    let (x, y) = self.cursor_doc();
                    if let Some(i) = self.model.zone_at(x, y) {
                        self.undo.push(self.model.zones.clone());
                        self.redo.clear();
                        let z = self.model.zones.zones.remove(i);
                        self.model.regroup();
                        if self.model.live_cut {
                            self.selected.clear();
                        }
                        self.message = format!("zone \"{}\" deleted", z.name);
                    }
                }
                Key::Character(c @ ("[" | "]")) => {
                    let (x, y) = self.cursor_doc();
                    if let Some(i) = self.model.zone_at(x, y) {
                        let j = if c == "[" { i.checked_sub(1) } else { Some(i + 1) };
                        if let Some(j) = j.filter(|&j| j < self.model.zones.zones.len()) {
                            self.undo.push(self.model.zones.clone());
                            self.redo.clear();
                            self.model.zones.zones.swap(i, j);
                            self.model.regroup();
                        if self.model.live_cut {
                            self.selected.clear();
                        }
                            self.message = format!(
                                "zone \"{}\" is now number {} of {}",
                                self.model.zones.zones[j].name,
                                j + 1,
                                self.model.zones.zones.len()
                            );
                        }
                    }
                }
                Key::Character("g") => {
                    self.look = match self.look {
                        Look::Picture => Look::Mix,
                        Look::Mix => Look::Groups,
                        Look::Groups => Look::Picture,
                    };
                }
                Key::Character("d") => self.show_doubts = !self.show_doubts,
                Key::Character("n") => self.jump_to_doubt(1),
                Key::Character("N") => self.jump_to_doubt(-1),
                Key::Character("f") => self.fit(),
                Key::Character("c") => {
                    self.config.cut = !self.config.cut;
                    self.model.live_cut =
                        self.config.cut && self.model.base.pieces.len() <= LIVE_CUT_MAX_PIECES;
                    self.model.regroup();
                        if self.model.live_cut {
                            self.selected.clear();
                        }
                    self.selected.clear();
                    self.message = format!(
                        "cut along zones when saving: {}",
                        if self.config.cut { "on" } else { "off" }
                    );
                }
                Key::Character("s") => self.save(),
                Key::Character("?") => eprintln!(
                    "z zone, Enter close, Backspace undo point, Esc cancel; Cmd+Z / u undo, Cmd+Shift+Z / U redo; click / Shift+click select pieces, Enter name them; x delete zone; [ ] zone order; g look; d doubts; n N next doubtful; f fit; c cut on save; s save; q quit"
                ),
                _ => {}
            },
        }
        self.redraw_soon();
    }

    fn draw(&mut self) {
        let cursor = self.cursor_doc();
        let status = self.status_line();
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let m = &self.model;
        let v = &self.view;
        let ui = self.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
        let font = Font::from_typeface(&self.font_regular, 13.0 * ui);
        let look = self.look;
        let show_doubts = self.show_doubts;
        let hovered = self.hovered;
        let tool = &self.tool;
        let selected = &self.selected;
        let message = match tool {
            Tool::Naming(_, name) | Tool::NamingSelection(name) => format!("name: {name}_"),
            _ => self.message.clone(),
        };
        let bar = 44.0 * ui;
        let result = surface.present(&mut |canvas: &Canvas, _| {
            canvas.clear(Color::from_rgb(38, 38, 40));
            canvas.save();
            canvas.translate((v.x, v.y));
            canvas.scale((v.scale, v.scale));
            let mut fill = Paint::default();
            fill.set_anti_alias(true);
            for (i, path) in m.paths.iter().enumerate() {
                let Some(path) = path else { continue };
                let piece = &m.doc.pieces[i];
                let colour = match look {
                    Look::Picture | Look::Mix => m.fills[i],
                    Look::Groups if piece.color == 0 => Color::BLACK,
                    Look::Groups => m.groups[i].unwrap_or(Color::from_rgb(119, 119, 119)),
                };
                fill.set_color(colour);
                canvas.draw_path(path, &fill);
                if look == Look::Mix
                    && piece.color != 0
                    && let Some(g) = m.groups[i]
                {
                    fill.set_color(g.with_a(130));
                    canvas.draw_path(path, &fill);
                }
            }
            let px = 1.0 / v.scale;
            let mut stroke = Paint::default();
            stroke.set_anti_alias(true);
            stroke.set_style(PaintStyle::Stroke);
            if show_doubts {
                stroke.set_stroke_width(2.0 * ui * px);
                let index: std::collections::HashMap<u32, Doubt> =
                    m.report.entries.iter().map(|e| (e.id, e.doubt)).collect();
                for (i, piece) in m.doc.pieces.iter().enumerate() {
                    if let (Some(d), Some(path)) = (index.get(&piece.id), &m.paths[i]) {
                        stroke.set_color(match d {
                            Doubt::Unnamed => Color::from_rgb(255, 40, 40),
                            Doubt::Split => Color::from_rgb(255, 212, 0),
                        });
                        canvas.draw_path(path, &stroke);
                    }
                }
            }
            stroke.set_stroke_width(1.5 * ui * px);
            stroke.set_color(Color::from_rgb(0, 220, 255));
            for path in m.zone_paths.iter().flatten() {
                canvas.draw_path(path, &stroke);
            }
            fill.set_color(Color::from_argb(150, 255, 140, 0));
            stroke.set_color(Color::from_rgb(255, 140, 0));
            stroke.set_stroke_width(2.0 * ui * px);
            for path in selected.iter().filter_map(|&i| m.paths[i].as_ref()) {
                canvas.draw_path(path, &fill);
                canvas.draw_path(path, &stroke);
            }
            if let Some(path) = hovered.and_then(|i| m.paths[i].as_ref()) {
                stroke.set_color(Color::WHITE);
                stroke.set_stroke_width(2.0 * ui * px);
                canvas.draw_path(path, &stroke);
            }
            let points = match tool {
                Tool::Drawing(p) | Tool::Naming(p, _) => Some(p),
                Tool::Idle | Tool::NamingSelection(_) => None,
            };
            if let Some(points) = points {
                stroke.set_color(Color::from_rgb(255, 120, 0));
                stroke.set_stroke_width(2.0 * ui * px);
                fill.set_color(Color::from_rgb(255, 120, 0));
                for w in points.windows(2) {
                    canvas.draw_line(w[0], w[1], &stroke);
                }
                if let Some(&last) = points.last() {
                    let end = if matches!(tool, Tool::Drawing(_)) { cursor } else { points[0] };
                    canvas.draw_line(last, end, &stroke);
                }
                for &p in points {
                    canvas.draw_circle(p, 4.0 * ui * px, &fill);
                }
            }
            canvas.restore();

            // zone names at the top left of each zone
            let mut text = Paint::default();
            text.set_anti_alias(true);
            for (z, path) in m.zones.zones.iter().zip(&m.zone_paths) {
                let Some(path) = path else { continue };
                let b = path.bounds();
                let (sx, sy) = (v.x + b.left * v.scale, v.y + b.top * v.scale);
                text.set_color(Color::from_argb(200, 0, 0, 0));
                canvas.draw_str(&z.name, (sx + 4.0 * ui + 1.0, sy + 15.0 * ui + 1.0), &font, &text);
                text.set_color(Color::from_rgb(0, 220, 255));
                canvas.draw_str(&z.name, (sx + 4.0 * ui, sy + 15.0 * ui), &font, &text);
            }

            // status bar
            let size = canvas.base_layer_size();
            let (w, h) = (size.width as f32, size.height as f32);
            fill.set_color(Color::from_argb(235, 20, 20, 22));
            canvas.draw_rect(Rect::from_xywh(0.0, h - bar, w, bar), &fill);
            text.set_color(Color::from_rgb(230, 230, 230));
            canvas.draw_str(&status, (10.0 * ui, h - bar + 17.0 * ui), &font, &text);
            text.set_color(Color::from_rgb(255, 190, 90));
            canvas.draw_str(&message, (10.0 * ui, h - bar + 35.0 * ui), &font, &text);
        });
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
    }

    fn status_line(&self) -> String {
        let m = &self.model;
        let piece = self.hovered.map_or_else(String::new, |i| {
            let p = &m.doc.pieces[i];
            format!(
                "piece {} {}   ",
                p.id,
                if p.group.is_empty() { "(no group)" } else { &p.group }
            )
        });
        let look = match self.look {
            Look::Picture => "picture",
            Look::Mix => "mix",
            Look::Groups => "groups",
        };
        format!(
            "{piece}{} of {} grouped   {} unnamed   {} split   {} zones   look {look}   cut on save {}",
            m.grouped(),
            m.doc.pieces.len(),
            m.report.unnamed,
            m.report.split,
            m.zones.zones.len(),
            if self.config.cut { "on" } else { "off" }
        )
    }
}

impl ApplicationHandler for Editor {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let title = format!("fframes-vector editor: {}", self.config.doc.display());
        let attributes = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(1400, 900));
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
        self.window = Some(window);
        event_loop.set_control_flow(ControlFlow::Wait);
        self.redraw_soon();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(Err(e)) = self.surface.as_mut().map(Surface::resize) {
                    self.error = Some(e.to_string());
                    return event_loop.exit();
                }
                if !self.fitted {
                    self.fit();
                    self.fitted = true;
                }
                self.redraw_soon();
            }
            WindowEvent::RedrawRequested => {
                if !self.fitted {
                    self.fit();
                    self.fitted = true;
                }
                self.draw();
                if self.error.is_some() {
                    event_loop.exit();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } => self.handle_key(event_loop, &event),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(from) = self.dragging {
                    self.view.x += (position.x - from.x) as f32;
                    self.view.y += (position.y - from.y) as f32;
                    self.dragging = Some(position);
                }
                self.cursor = position;
                let (x, y) = self.cursor_doc();
                self.hovered = self.model.piece_at(x, y);
                self.redraw_soon();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                match (button, &mut self.tool) {
                    (MouseButton::Left, Tool::Drawing(points)) if pressed => {
                        let p = self.view.to_doc(self.cursor.x as f32, self.cursor.y as f32);
                        points.push(p);
                    }
                    (MouseButton::Left, Tool::Idle) => {
                        self.dragging = pressed.then_some(self.cursor);
                        if pressed {
                            self.pressed_at = Some(self.cursor);
                        } else if let Some(at) = self.pressed_at.take()
                            && (at.x - self.cursor.x).hypot(at.y - self.cursor.y) < 4.0
                            && let Some(i) = self.hovered
                        {
                            // a click, not a drag: select (Shift adds or removes)
                            if self.modifiers.shift_key() {
                                match self.selected.iter().position(|&j| j == i) {
                                    Some(k) => {
                                        self.selected.remove(k);
                                    }
                                    None => self.selected.push(i),
                                }
                            } else {
                                self.selected = vec![i];
                            }
                            self.message = format!(
                                "{} pieces selected: Enter to name them, Shift+click to add, Esc to clear",
                                self.selected.len()
                            );
                        }
                    }
                    (MouseButton::Left | MouseButton::Right | MouseButton::Middle, _) => {
                        self.dragging = pressed.then_some(self.cursor);
                    }
                    _ => {}
                }
                self.redraw_soon();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (cx, cy) = (self.cursor.x as f32, self.cursor.y as f32);
                let zoom = self.modifiers.super_key() || self.modifiers.control_key();
                match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => {
                        if zoom || dx == 0.0 {
                            self.view.zoom_at(1.15_f32.powf(dy), cx, cy);
                        } else {
                            self.view.x += dx * 40.0;
                            self.view.y += dy * 40.0;
                        }
                    }
                    MouseScrollDelta::PixelDelta(p) => {
                        if zoom {
                            self.view.zoom_at((p.y as f32 / 200.0).exp(), cx, cy);
                        } else {
                            self.view.x += p.x as f32;
                            self.view.y += p.y as f32;
                        }
                    }
                }
                self.redraw_soon();
            }
            WindowEvent::PinchGesture { delta, .. } => {
                let (cx, cy) = (self.cursor.x as f32, self.cursor.y as f32);
                self.view.zoom_at(1.0 + delta as f32, cx, cy);
                self.redraw_soon();
            }
            _ => {}
        }
    }
}

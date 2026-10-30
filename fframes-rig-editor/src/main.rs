//! `fframes-rig-editor figure.glb [--rig figure.rig.json] [--source video.mp4 --options "--height 1.7 --hull"] [--solid figure.solid]`
//!
//! The rig editor. The figure stands in a plain room; the camera turns around it. Cuts take
//! away what the reconstruction joined (an arm to the body), joints mark where the figure
//! bends. Both are drawn on planes through the figure and kept in the rig file (default
//! `figure.rig.json` beside the model); the model itself is never written.
//!
//! 1. Click a plane icon (transverse, coronal, sagittal): the plane appears through the
//!    figure with the model's cross-section on it. Move it with the arrow keys (`Up`/`Down`,
//!    Shift finer, Alt coarser) or Alt+scroll.
//! 2. Click dots around the area to carve; drag a dot to move it, `Backspace` removes the last.
//!    `Enter` closes the contour: it becomes a section of the cut at that position.
//! 3. Move the plane and draw again where the shape changes: the cut is interpolated between
//!    its sections. `[` / `]` shorten or lengthen its reach below the first section,
//!    Shift+`[` / Shift+`]` above the last. A second plane icon adds a view: the cut is where
//!    all its views overlap. `P` makes the cut a protected region (the shoulder): no cut
//!    removes anything there.
//! 4. `A` (or the Accept icon) keeps the cut and saves the rig file; `D` discards it. Nothing is
//!    kept before it is accepted.
//! 5. `J` (or the Joint icon), type a name (`elbow:shoulder` for a parent), `Enter`; click the
//!    joint on a plane, then on a second plane across it; `A` keeps it.
//! 6. `X` (Apply) rebuilds the model with the accepted cuts (`fframes-vector depth --cuts`) and
//!    shows it; `T` switches between the original and the rebuilt model.
//! 7. With the joints `shoulder` and `elbow:shoulder` accepted, `.` / `,` raise and lower the
//!    arm out to the side, `;` / `'` swing it forward and back, `0` puts it back. The arm is
//!    what can be reached from the elbow without the body; the shoulder blends.
//!
//! The box at the top right lists what is not finished: a cut drawn in one plane only, a
//! joint placed in one plane only, anything not accepted. The room's walls are square to
//! the figure: its front is found from its shape (widest across the shoulders, the way nearer
//! the turntable's first frame); `F` confirms it, `B` turns it round, Shift+`B` a quarter
//! turn. `Cmd+Z` / `Cmd+Shift+Z` undo and redo accepted
//! changes. Turn: drag. Zoom: scroll. Up and down: Shift+drag. `R` front view, `Q` quit.

use std::rc::Rc;
use std::sync::mpsc;

use fframes_native_player::fframes_skia_renderer::skia_safe::{
    self, AlphaType, Color, ColorType, Data, Font, FontMgr, FontStyle, ImageInfo, Paint, PaintStyle, Point, Rect,
    SamplingOptions,
};
use fframes_native_player::winit;
use fframes_native_player::{PlayerBackend, Surface};
use fframes_rig::{
    Cut, Joint, Observation, Plane, Preset, RigFile, Section, Source, V3, View, axes, limb_weights, pose, rig_path, rotation,
    Draft, draft_path, propose_cuts, propose_skeleton, section_segments, shape_front,
};
use fframes_three_d::{
    Background, Camera, Environment, Geometry, Light, Mat3, Material, Model, Node, Quat, Renderer, Rgb, Scene, Vec3,
    Vec4, vec3,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

const SYSTEM_FONTS: &[&str] = &[".AppleSystemUIFont", "Helvetica Neue", "Segoe UI", "DejaVu Sans"];
/// Vertical field of view, degrees.
const FOV: f32 = 35.0;
/// A press that moves less than this (pixels) is a click, not a drag.
const CLICK: f64 = 4.0;
/// A press this close to a dot (pixels) moves the dot.
const GRAB: f32 = 9.0;
/// Rendered share of the window's pixels.
const QUALITY: f32 = 0.75;

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fframes-rig-editor: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// A figure: its scene node tree and its triangles in model coordinates.
struct Figure {
    model: Model,
    positions: Vec<V3>,
    indices: Vec<u32>,
}

fn load_figure(path: &str) -> Result<Figure, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let model = Model::from_gltf(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let (mut positions, mut indices) = (Vec::new(), Vec::new());
    for (id, node) in model.scene.nodes() {
        if let Some(mesh) = node.as_mesh() {
            let m = model.scene.world_matrix(id);
            let base = positions.len() as u32;
            positions.extend(mesh.geometry.positions.iter().map(|p| m.transform_point3(*p).to_array()));
            indices.extend(mesh.geometry.indices.iter().map(|i| i + base));
        }
    }
    if indices.is_empty() {
        return Err(format!("{path}: no triangles"));
    }
    Ok(Figure { model, positions, indices })
}

/// The figure in a plain room: a box around it, its walls square to the figure's front
/// (`front_yaw`, degrees) and centred on it, lit evenly.
fn room_scene(figure: &Figure, lo: Vec3, centre: Vec3, size: f32, front_yaw: f32) -> Scene {
    let mut scene = Scene::new()
        .environment(Environment::studio().intensity(0.7))
        .background(Background::Color(Rgb::hex(0x20_22_28)));
    figure.model.add_to(&mut scene, None);
    let wall = Material::standard(0x6e_70_76).roughness(0.95);
    let floor = Material::standard(0x55_57_5c).roughness(0.9);
    let (h, t) = (size * 0.7, 0.05);
    let turn = front_yaw.to_radians();
    let (side, _, front) = axes(front_yaw);
    // a point of the room's own plan (x along the figure's side, z along its front)
    let place_x = |x: f32, z: f32| centre.x + x * side[0] + z * front[0];
    let place_z = |x: f32, z: f32| centre.z + x * side[2] + z * front[2];
    scene.add(Node::mesh(Geometry::floor(size, size).rotate_y(turn).translate(centre.x, lo.y, centre.z), floor).shadows(false, true));
    for (x, z, w, d) in [(0.0, -size / 2.0, size, t), (0.0, size / 2.0, size, t), (-size / 2.0, 0.0, t, size), (size / 2.0, 0.0, t, size)] {
        let g = Geometry::cuboid(w, h, d).rotate_y(turn).translate(place_x(x, z), lo.y + h / 2.0, place_z(x, z));
        scene.add(Node::mesh(g, wall.clone()).shadows(false, true));
    }
    scene.add(Node::mesh(Geometry::cuboid(size, t, size).rotate_y(turn).translate(centre.x, lo.y + h, centre.z), wall).shadows(false, false));
    scene.add(Node::light(Light::hemisphere(Rgb::hex(0xd8_dc_e4), Rgb::hex(0x40_3c_38), 0.5)));
    scene.add(Node::light(Light::sun(Rgb::WHITE, 1.8, vec3(1.5, 3.0, 2.5))));
    scene.add(Node::light(Light::sun(Rgb::hex(0xdd_e4_ff), 0.6, vec3(-2.0, 1.5, -2.5))));
    scene
}

/// The arm turned about the shoulder: each vertex's weight, the joints, the angles
/// (degrees: out to the side, forward) and the scene drawn with it.
struct Posed {
    weights: Vec<f32>,
    shoulder: V3,
    elbow: V3,
    side: f32,
    forward: f32,
    scene: Option<Scene>,
}

/// A dot being dragged: of the open contour, or of a section of the cut being edited
/// (view, section, point).
#[derive(Clone, Copy)]
enum Grab {
    Draft(usize),
    Section(usize, usize, usize),
    /// A joint (index in the rig), moved across the screen at its depth.
    Joint(usize),
    /// A section's outline (view, section), or with `whole` the entire cut, moved across the
    /// screen; the last point under the pointer, on the screen plane through the grab.
    Outline { view: usize, section: usize, whole: bool, last: [f32; 3] },
}

/// Sections closer than this to the plane's position are on it, metres.
const ON_PLANE: f32 = 0.0005;

/// 3x3 matrix product.
fn mul(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}

/// The plane being worked on.
#[derive(Clone, Copy)]
struct Active {
    preset: Preset,
    plane: Plane,
    offset: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Plane(Preset),
    Protect,
    Joint,
    Accept,
    Discard,
    Undo,
    Apply,
    Show,
}

struct Editor {
    model_path: String,
    figure: Figure,
    /// The figure rebuilt with the cuts, when applied.
    rebuilt: Option<Figure>,
    showing_rebuilt: bool,
    scenes: (Scene, Option<Scene>),
    lo: Vec3,
    hi: Vec3,
    centre: Vec3,
    height: f32,
    room: f32,
    target: Vec3,
    distance: f32,
    /// Degrees: 0 looks from +z, 90 from +x.
    azimuth: f32,
    elevation: f32,
    rig: RigFile,
    rig_file: std::path::PathBuf,
    history: Vec<RigFile>,
    future: Vec<RigFile>,
    active: Option<Active>,
    /// The cut being drawn, not yet accepted.
    pending: Option<Cut>,
    /// The contour being drawn on the active plane, its dots in plane coordinates.
    draft: Vec<[f32; 2]>,
    grabbed: Option<Grab>,
    /// An accepted cut taken out for editing: its place in the list and how it was.
    editing: Option<(usize, Cut)>,
    /// The joint last clicked (the arm raise uses the selected shoulder).
    selected: Option<String>,
    /// Steps of the unaccepted work (the cut being drawn or edited, the open contour, the joint
    /// being placed), each saved before a change, for undo.
    steps: Vec<(Option<Cut>, Vec<[f32; 2]>, Option<Joint>)>,
    /// Open contours left on other plane positions (direction, position, dots).
    parked: Vec<(Preset, f32, Vec<[f32; 2]>)>,
    /// Where unaccepted work is saved after every change.
    draft_file: std::path::PathBuf,
    /// When the window last came to the front; the click that did it places nothing.
    focused_at: Option<std::time::Instant>,
    activation_click: bool,
    /// The joint being placed, not yet accepted.
    joint: Option<Joint>,
    /// A joint name being typed.
    typing: Option<String>,
    /// Metres per voxel of the reconstruction, when its solid is known.
    voxel: Option<f32>,
    applying: Option<mpsc::Receiver<Result<String, String>>>,
    posed: Option<Posed>,
    message: String,
    quit_armed: bool,
    buttons: Vec<(Rect, Action)>,
    renderer: Option<(u32, u32, Renderer)>,
    window: Option<Rc<Window>>,
    surface: Option<Surface>,
    typeface: skia_safe::Typeface,
    cursor: PhysicalPosition<f64>,
    pressed: Option<(PhysicalPosition<f64>, PhysicalPosition<f64>)>,
    modifiers: ModifiersState,
    error: Option<String>,
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "fframes-rig-editor figure.glb [--rig figure.rig.json] [--source video.mp4 --options \"--height 1.7 --hull\"] [--solid figure.solid]";
    let model_path = args.first().filter(|a| !a.starts_with("--")).ok_or(usage)?.clone();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let rig_file = value("--rig").map_or_else(|| rig_path(std::path::Path::new(&model_path)), std::path::PathBuf::from);
    let figure = load_figure(&model_path)?;
    let mut rig = if rig_file.exists() {
        RigFile::load(&rig_file)?
    } else {
        RigFile { model: model_path.clone(), ..RigFile::default() }
    };
    if let Some(video) = value("--source") {
        let options = value("--options").map(|o| o.split_whitespace().map(String::from).collect()).unwrap_or_default();
        rig.source = Some(Source { video, options });
    }
    // the voxel size from the solid's first line ("metres_per_voxel": 0.0068)
    let voxel = value("--solid").and_then(|path| {
        let text = std::fs::read(&path).ok()?;
        let head = String::from_utf8_lossy(&text[..text.iter().position(|&b| b == b'\n')?]).to_string();
        let at = head.find("\"metres_per_voxel\":")? + 19;
        head[at..].split([',', '}']).next()?.trim().parse::<f32>().ok()
    });
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in &figure.positions {
        lo = lo.min(Vec3::from_array(*p));
        hi = hi.max(Vec3::from_array(*p));
    }
    let height = (hi.y - lo.y).max(0.1);
    let centre = (lo + hi) * 0.5;
    let room = height * 7.0;
    eprintln!(
        "{model_path}: {} vertices, {} triangles, {height:.2} m high; rig {} ({} cuts, {} joints){}",
        figure.positions.len(),
        figure.indices.len() / 3,
        rig_file.display(),
        rig.cuts.len(),
        rig.joints.len(),
        voxel.map_or_else(String::new, |v| format!("; voxel {:.1} mm", v * 1000.0))
    );
    let font_mgr = FontMgr::new();
    let typeface = SYSTEM_FONTS
        .iter()
        .find_map(|f| font_mgr.match_family_style(f, FontStyle::normal()))
        .or_else(|| font_mgr.legacy_make_typeface(None, FontStyle::normal()))
        .ok_or("no system font available")?;
    // the room square to the figure: its front from its shape until the user confirms one
    if !rig.front_confirmed {
        rig.front_yaw = shape_front(&figure.positions, &figure.indices);
    }
    let base = room_scene(&figure, lo, centre, room, rig.front_yaw);
    // unaccepted work from last time
    let draft_file = draft_path(&rig_file);
    let draft = if draft_file.exists() { Draft::load(&draft_file)? } else { Draft::default() };
    let mut editor = Editor {
        model_path,
        figure,
        rebuilt: None,
        showing_rebuilt: false,
        scenes: (base, None),
        lo,
        hi,
        centre,
        height,
        room,
        target: centre,
        distance: height * 2.2,
        azimuth: rig.front_yaw,
        elevation: 8.0,
        rig,
        rig_file,
        history: Vec::new(),
        future: Vec::new(),
        active: None,
        pending: None,
        draft: Vec::new(),
        grabbed: None,
        editing: None,
        selected: None,
        steps: Vec::new(),
        parked: Vec::new(),
        draft_file,
        focused_at: None,
        activation_click: false,
        joint: None,
        typing: None,
        voxel,
        applying: None,
        posed: None,
        message: String::new(),
        quit_armed: false,
        buttons: Vec::new(),
        renderer: None,
        window: None,
        surface: None,
        typeface,
        cursor: PhysicalPosition::default(),
        pressed: None,
        modifiers: ModifiersState::empty(),
        error: None,
    };
    // proposals, until the rig has its own: a skeleton, and cuts where an arm joins the body
    if editor.rig.joints.is_empty() {
        editor.rig.joints = propose_skeleton(&editor.figure.positions, &editor.figure.indices, editor.rig.front_yaw);
        eprintln!("proposed: a skeleton of {} joints", editor.rig.joints.len());
    }
    // a proposal of the earlier kind (three views) is replaced by the single box, and an
    // unaccepted edit of it with it
    let mut draft = draft;
    let old: Vec<usize> = (0..editor.rig.cuts.len()).filter(|&i| editor.rig.cuts[i].proposed && editor.rig.cuts[i].views.len() > 1).collect();
    if !old.is_empty() {
        // an edit already of the single box is kept; only an edit of the old kind goes
        if draft.editing.is_some_and(|i| old.contains(&i)) && draft.cut.as_ref().is_some_and(|c| c.views.len() > 1) {
            draft.cut = None;
            draft.editing = None;
        }
        editor.rig.cuts.retain(|c| !(c.proposed && c.views.len() > 1));
        eprintln!("the earlier three-view proposal is replaced by one box");
    }
    if editor.rig.cuts.is_empty() {
        editor.rig.cuts = propose_cuts(&editor.figure.positions, &editor.figure.indices, editor.rig.front_yaw);
        for c in &editor.rig.cuts {
            let range = c.views.first().and_then(View::range).unwrap_or_default();
            eprintln!("proposed: {} from {:.3} to {:.3} m", c.name, range.0, range.1);
        }
    }
    // the replacement is kept, so it happens once
    if !old.is_empty() {
        editor.rig.save(&editor.rig_file)?;
    }
    if !draft.is_empty() {
        editor.editing = draft.editing.and_then(|i| editor.rig.cuts.get(i).cloned().map(|c| (i, c)));
        editor.pending = draft.cut;
        editor.joint = draft.joint;
        editor.parked = draft.contours;
        editor.message = format!("unaccepted work from last time restored ({})", editor.draft_file.display());
    }
    for task in editor.rig.open_tasks() {
        eprintln!("open: {task}");
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.run_app(&mut editor).map_err(|e| e.to_string())?;
    editor.error.map_or(Ok(()), Err)
}

/// The narrowest width of a closed contour (over 36 directions), metres.
fn narrowest(contour: &[[f32; 2]]) -> f32 {
    (0..36)
        .map(|k| {
            let a = (k as f32 * 5.0).to_radians();
            let (c, s) = (a.cos(), a.sin());
            let along: Vec<f32> = contour.iter().map(|p| p[0] * c + p[1] * s).collect();
            along.iter().copied().fold(f32::MIN, f32::max) - along.iter().copied().fold(f32::MAX, f32::min)
        })
        .fold(f32::MAX, f32::min)
}

impl Editor {
    fn camera(&self) -> Camera {
        Camera::perspective(FOV).orbit(self.target, self.distance, self.azimuth, self.elevation).clip(0.01, self.room * 2.0)
    }

    fn shown(&self) -> &Figure {
        match (&self.rebuilt, self.showing_rebuilt) {
            (Some(r), true) => r,
            _ => &self.figure,
        }
    }

    /// The ray under a window position: origin and unit direction.
    fn ray(&self, at: PhysicalPosition<f64>) -> Option<(V3, V3)> {
        let size = self.window.as_ref()?.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        let ndc = (2.0 * at.x as f32 / w - 1.0, 1.0 - 2.0 * at.y as f32 / h);
        let inverse = self.camera().view_projection(w / h).inverse();
        let unproject = |z: f32| {
            let p = inverse * Vec4::new(ndc.0, ndc.1, z, 1.0);
            p.truncate() / p.w
        };
        let (near, far) = (unproject(0.0), unproject(1.0));
        Some((near.to_array(), (far - near).normalize().to_array()))
    }

    /// The active plane's point under a window position, in plane coordinates.
    fn plane_point(&self, at: PhysicalPosition<f64>) -> Option<[f32; 2]> {
        let a = self.active?;
        let (o, d) = self.ray(at)?;
        a.plane.hit(a.offset, o, d).map(|(p, _)| a.plane.to_2d(p))
    }

    /// A model point on the window, pixels.
    fn screen(&self, p: V3) -> Option<Point> {
        let size = self.window.as_ref()?.inner_size();
        self.camera().project(Vec3::from_array(p), size.width as f32, size.height as f32).map(|s| Point::new(s.x, s.y))
    }

    /// The rig as it would be with the pending cut and joint accepted.
    fn with_pending(&self) -> RigFile {
        let mut rig = self.rig.clone();
        if let Some(c) = &self.pending {
            match self.editing {
                Some((i, _)) if i < rig.cuts.len() => rig.cuts[i] = c.clone(),
                _ => rig.cuts.push(c.clone()),
            }
        }
        if let Some(j) = &self.joint {
            rig.joints.retain(|q| q.name != j.name);
            rig.joints.push(j.clone());
        }
        rig
    }

    /// What is not finished, for the box at the top right.
    fn alerts(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.draft.is_empty() {
            out.push(format!("contour with {} dots not closed (Enter)", self.draft.len()));
        }
        if let Some(c) = &self.pending {
            out.push(format!("{} not accepted (A keeps, D discards)", c.name));
        }
        if let Some(j) = &self.joint {
            out.push(format!("joint {} not accepted (A keeps, D discards)", j.name));
        }
        out.extend(self.with_pending().open_tasks());
        out
    }

    fn remember(&mut self) {
        self.history.push(self.rig.clone());
        self.future.clear();
    }

    fn save(&mut self) {
        match self.rig.save(&self.rig_file) {
            Ok(()) => {
                self.message = format!("saved {}", self.rig_file.display());
            }
            Err(e) => self.message = e,
        }
    }

    /// Before the plane moves or changes: an open contour of three or more dots becomes a
    /// section (closed), a shorter one is kept for this plane position and comes back there.
    fn leave_plane(&mut self) {
        let Some(a) = self.active else { return };
        if self.draft.len() >= 3 {
            self.close_contour();
            self.message = format!("the open contour was closed and kept as a section at {:.3} m", a.offset);
        } else if !self.draft.is_empty() {
            let dots = std::mem::take(&mut self.draft);
            self.parked.push((a.preset, a.offset, dots));
            self.message = format!("{} open dots kept on the {} plane at {:.3} m", self.parked.last().map_or(0, |p| p.2.len()), a.preset.name(), a.offset);
        }
    }

    /// After the plane moved or changed: open dots kept at this position come back.
    fn arrive_plane(&mut self) {
        let Some(a) = self.active else { return };
        if let Some(i) = self.parked.iter().position(|(p, o, _)| *p == a.preset && (o - a.offset).abs() <= ON_PLANE) {
            self.draft = self.parked.remove(i).2;
            self.message = format!("{} open dots back; Enter or a click on the first dot closes", self.draft.len());
        }
    }

    fn choose_plane(&mut self, preset: Preset) {
        let plane = preset.plane(self.rig.front_yaw);
        // keep the position when the same direction is chosen again
        let offset = match self.active {
            Some(a) if a.preset == preset => a.offset,
            _ => plane.offset_of(self.centre.to_array()),
        };
        self.leave_plane();
        let kept = std::mem::take(&mut self.message);
        self.active = Some(Active { preset, plane, offset });
        self.message = format!("{} plane at {:.3} m; click dots around the area, Enter closes   {kept}", preset.name(), offset);
        self.arrive_plane();
    }

    fn move_plane(&mut self, by: f32) {
        if self.active.is_none() {
            return;
        }
        self.leave_plane();
        if let Some(a) = &mut self.active {
            a.offset += by;
            if self.draft.is_empty() && !self.message.contains("kept") {
                self.message = format!("{} plane at {:.3} m", a.preset.name(), a.offset);
            }
        }
        self.arrive_plane();
    }

    /// Where the pointer is on the screen plane through `at` (square to the view): the point a
    /// dragged dot follows, one to one with the pointer.
    fn screen_drag(&self, position: PhysicalPosition<f64>, at: V3) -> Option<V3> {
        let camera = self.camera();
        let screen = Plane { normal: camera.forward().to_array(), u: camera.right().to_array(), v: camera.up_vector().to_array() };
        let (o, d) = self.ray(position)?;
        screen.hit(screen.offset_of(at), o, d).map(|(p, _)| p)
    }

    /// Zooms by `factor` (under 1 closer) towards the point of the figure under the pointer,
    /// or the middle of the view where there is none.
    fn zoom(&mut self, factor: f32) {
        let factor = factor.clamp(0.5, 2.0);
        let wanted = (self.distance * factor).clamp(self.height * 0.05, self.height * 3.0);
        let factor = wanted / self.distance;
        if let Some((o, d)) = self.ray(self.cursor) {
            let f = self.shown();
            if let Some((hit, _)) = fframes_rig::pick(&f.positions, &f.indices, o, d) {
                // keep the point under the pointer where it is: the view's middle moves
                // towards it as much as the camera comes closer
                let hit = Vec3::from_array(hit);
                self.target += (hit - self.target) * (1.0 - factor);
            }
        }
        self.distance = wanted;
    }

    /// Writes everything not yet accepted to the draft file (removed when nothing is left).
    fn autosave(&mut self) {
        let mut contours = self.parked.clone();
        if let (Some(a), false) = (self.active, self.draft.is_empty()) {
            contours.push((a.preset, a.offset, self.draft.clone()));
        }
        let draft = Draft { cut: self.pending.clone(), editing: self.editing.as_ref().map(|e| e.0), joint: self.joint.clone(), contours };
        if let Err(e) = draft.save(&self.draft_file) {
            self.message = format!("could not save the draft: {e}");
        }
    }

    fn close_contour(&mut self) {
        let Some(a) = self.active else { return };
        if self.draft.len() < 3 {
            self.message = "a contour needs at least 3 dots".into();
            return;
        }
        self.keep_step();
        let contour = std::mem::take(&mut self.draft);
        if let Some(v) = self.voxel {
            let w = narrowest(&contour);
            if w < 2.0 * v {
                self.message = format!(
                    "warning: the contour is {:.1} mm across at its narrowest, under two voxels ({:.1} mm): it may leave no gap after the surface is built",
                    w * 1000.0,
                    2000.0 * v
                );
            }
        }
        let n = self.rig.cuts.len() + 1;
        let cut = self.pending.get_or_insert_with(|| Cut { name: format!("cut {n}"), ..Cut::default() });
        let view = match cut.views.iter().position(|v| v.plane.parallel_to(&a.plane)) {
            Some(i) => &mut cut.views[i],
            None => {
                cut.views.push(View::new(a.plane));
                cut.views.last_mut().expect("just pushed")
            }
        };
        view.sections.retain(|s| (s.offset - a.offset).abs() > 1e-4);
        view.sections.push(Section { offset: a.offset, contour });
        view.sort();
        let sections = view.sections.len();
        if !self.message.starts_with("warning") {
            self.message = format!("{}: section {sections} in the {} plane at {:.3} m; move the plane to add another, A accepts", cut.name, a.preset.name(), a.offset);
        }
    }

    fn accept(&mut self) {
        // an open contour is closed by accepting, as by Enter
        if self.draft.len() >= 3 {
            self.close_contour();
        } else if !self.draft.is_empty() {
            self.message = "a contour needs at least 3 dots (Esc drops them)".into();
            return;
        }
        if self.pending.is_none() && self.joint.is_none() {
            // nothing being drawn: accept the proposed skeleton as it stands
            let proposed = self.rig.joints.iter().filter(|j| j.proposed).count();
            if proposed > 0 {
                self.remember();
                for j in &mut self.rig.joints {
                    j.proposed = false;
                }
                self.posed = None;
                self.save();
                self.message = format!("skeleton accepted ({proposed} joints)");
            } else if self.rig.cuts.iter().any(|c| c.proposed) {
                self.message = "drag a dot of a proposed cut (magenta) to open it, or E, then A".into();
            } else {
                self.message = "nothing to accept".into();
            }
            return;
        }
        self.remember();
        self.steps.clear();
        if let Some(mut c) = self.pending.take() {
            c.proposed = false;
            self.message = format!("{} accepted", c.name);
            // an edited cut replaces the one it was opened from
            match self.editing.take() {
                Some((i, _)) if i < self.rig.cuts.len() => self.rig.cuts[i] = c,
                _ => self.rig.cuts.push(c),
            }
        }
        if let Some(j) = self.joint.take() {
            self.message = format!("joint {} accepted", j.name);
            self.rig.joints.retain(|q| q.name != j.name);
            self.rig.joints.push(j);
        }
        // joints or cuts changed: the arm's weights are found again on the next turn
        self.posed = None;
        self.save();
    }

    fn discard(&mut self) {
        self.draft.clear();
        self.steps.clear();
        let had_changes = match (&self.pending, &self.editing) {
            (Some(p), Some((_, original))) => p != original,
            _ => false,
        };
        let had = self.pending.take().is_some() | self.joint.take().is_some();
        self.message = match (self.editing.take(), had) {
            // a proposed cut that was opened is dismissed altogether
            // a changed proposal goes back to how it was proposed; unchanged, it is dismissed
            (Some((i, c)), true) if c.proposed && had_changes => {
                self.pending = Some(c.clone());
                self.editing = Some((i, c.clone()));
                format!("{}: back to the proposal; D again dismisses it", c.name)
            }
            (Some((i, c)), _) if c.proposed => {
                self.remember();
                if i < self.rig.cuts.len() {
                    self.rig.cuts.remove(i);
                }
                self.save();
                format!("{} dismissed", c.name)
            }
            (Some((_, c)), _) => format!("{} left as it was", c.name),
            (None, true) => "discarded".into(),
            (None, false) => {
                // nothing open: the selected proposed joint is dismissed
                match self.selected.clone().and_then(|n| self.rig.joints.iter().position(|j| j.name == n && j.proposed)) {
                    Some(i) => {
                        self.remember();
                        let j = self.rig.joints.remove(i);
                        self.selected = None;
                        self.save();
                        format!("joint {} dismissed", j.name)
                    }
                    None => "nothing to discard".into(),
                }
            }
        };
    }

    /// Option+click on a box (a cut whose levels have the same corners): on one of its dots
    /// removes it (a dot of a middle level: that level; else that corner from every level),
    /// on an edge adds a dot there (on a level's outline: a corner on every level; on an edge
    /// between levels: a new level at that height).
    fn box_dot(&mut self, at: PhysicalPosition<f64>) {
        enum Change {
            Remove(usize, usize, usize),
            Corner(usize, usize, usize, [f32; 2]),
            Level(usize, f32, usize, [f32; 2]),
        }
        let at = Point::new(at.x as f32, at.y as f32);
        let ui = self.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
        let reach = GRAB * ui;
        let find = |cut: &Cut, me: &Self| -> Option<Change> {
            // on a segment: where along it (0..1), when the pointer is near
            let along = |p: V3, q: V3| -> Option<f32> {
                let (a, b) = (me.screen(p)?, me.screen(q)?);
                let ab = b - a;
                let t = (((at - a).x * ab.x + (at - a).y * ab.y) / (ab.x * ab.x + ab.y * ab.y).max(1e-6)).clamp(0.0, 1.0);
                ((a + ab * t - at).length() < reach).then_some(t)
            };
            for (vi, v) in cut.views.iter().enumerate().filter(|(_, v)| v.corresponding()) {
                for (l, s) in v.sections.iter().enumerate() {
                    for (k, q) in s.contour.iter().enumerate() {
                        if me.screen(v.plane.to_3d(*q, s.offset)).is_some_and(|p| (p - at).length() < reach) {
                            return Some(Change::Remove(vi, l, k));
                        }
                    }
                }
                for (l, s) in v.sections.iter().enumerate() {
                    let n = s.contour.len();
                    for k in 0..n {
                        let (p, q) = (s.contour[k], s.contour[(k + 1) % n]);
                        if let Some(t) = along(v.plane.to_3d(p, s.offset), v.plane.to_3d(q, s.offset)) {
                            return Some(Change::Corner(vi, l, k, [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]));
                        }
                        if let Some(up) = v.sections.get(l + 1) {
                            let r = up.contour[k];
                            if let Some(t) = along(v.plane.to_3d(p, s.offset), v.plane.to_3d(r, up.offset)) {
                                let offset = s.offset + (up.offset - s.offset) * t;
                                return Some(Change::Level(vi, offset, k, [p[0] + (r[0] - p[0]) * t, p[1] + (r[1] - p[1]) * t]));
                            }
                        }
                    }
                }
            }
            None
        };
        let change = match self.pending.as_ref().and_then(|c| find(c, self)) {
            Some(c) => {
                self.keep_step();
                c
            }
            None if self.pending.is_some() => {
                self.message = "Option+click a dot (removes it) or an edge (adds one) of the box being edited".into();
                return;
            }
            None => {
                let Some((i, c)) = (0..self.rig.cuts.len()).find_map(|i| find(&self.rig.cuts[i], self).map(|c| (i, c))) else {
                    self.message = "Option+click a dot of a box to remove it, or an edge to add one".into();
                    return;
                };
                self.keep_step();
                self.open_cut(i);
                c
            }
        };
        let Some(cut) = &mut self.pending else { return };
        self.message = match change {
            Change::Remove(vi, l, k) => {
                let v = &mut cut.views[vi];
                if l > 0 && l + 1 < v.sections.len() {
                    v.sections.remove(l);
                    "level removed".into()
                } else if v.remove_corner(k) {
                    "corner removed from every level".into()
                } else {
                    "a box keeps at least 3 corners".into()
                }
            }
            Change::Corner(vi, l, k, p) => {
                cut.views[vi].insert_corner(l, k, p);
                "corner added on every level: drag it".into()
            }
            Change::Level(vi, offset, k, p) => {
                cut.views[vi].insert_level(offset, k, p);
                format!("level added at {offset:.3} m: drag its dots")
            }
        };
    }

    /// Opens an accepted cut for editing (a copy; the accepted one stays until the edit is
    /// accepted): the one before the cut being edited, else the last.
    fn edit_cut(&mut self) {
        if self.pending.is_some() && self.editing.is_none() {
            self.message = "accept or discard the cut being drawn first".into();
            return;
        }
        if self.rig.cuts.is_empty() {
            self.message = "no accepted cut to edit".into();
            return;
        }
        let i = match self.editing.take() {
            Some((i, _)) => (i + self.rig.cuts.len() - 1) % self.rig.cuts.len(),
            None => self.rig.cuts.len() - 1,
        };
        self.open_cut(i);
    }

    fn open_cut(&mut self, i: usize) {
        let c = self.rig.cuts[i].clone();
        self.message = format!("editing {}: drag its dots on the plane at a section (N jumps to the next), A keeps, D leaves it", c.name);
        self.pending = Some(c.clone());
        self.editing = Some((i, c));
    }

    /// Moves the plane to the next (or previous) section of the cut being edited or drawn
    /// in the plane's direction.
    fn jump_section(&mut self, back: bool) {
        let (Some(a), Some(cut)) = (self.active, &self.pending) else {
            self.message = "choose a plane and a cut (E edits the last accepted one)".into();
            return;
        };
        let offsets: Vec<f32> = cut.views.iter().filter(|v| v.plane.parallel_to(&a.plane)).flat_map(|v| v.sections.iter().map(|s| s.offset)).collect();
        let next = if back {
            offsets.iter().copied().filter(|&o| o < a.offset - ON_PLANE).fold(None, |m: Option<f32>, o| Some(m.map_or(o, |m| m.max(o))))
        } else {
            offsets.iter().copied().filter(|&o| o > a.offset + ON_PLANE).fold(None, |m: Option<f32>, o| Some(m.map_or(o, |m| m.min(o))))
        };
        match next {
            Some(o) => {
                if let Some(a) = &mut self.active {
                    a.offset = o;
                }
                self.message = format!("at the section at {o:.3} m: drag its dots, Delete removes it");
            }
            None => self.message = "no further section in this plane direction".into(),
        }
    }

    /// Removes the section of the cut being edited or drawn that lies on the plane.
    fn delete_section(&mut self) {
        if self.active.is_none() || self.pending.is_none() {
            self.message = "Shift+Delete removes the section at the plane of the cut being edited".into();
            return;
        }
        self.keep_step();
        let (Some(a), Some(cut)) = (self.active, &mut self.pending) else { return };
        let mut removed = false;
        for v in cut.views.iter_mut().filter(|v| v.plane.parallel_to(&a.plane)) {
            let before = v.sections.len();
            v.sections.retain(|s| (s.offset - a.offset).abs() > ON_PLANE);
            removed |= v.sections.len() != before;
        }
        cut.views.retain(|v| !v.sections.is_empty());
        self.message = if removed { format!("section at {:.3} m removed", a.offset) } else { "no section on the plane here".into() };
    }

    /// The dot under a window position: of the open contour, else of a section on the plane
    /// of the cut being edited or drawn, else of an accepted cut's section on the plane (which
    /// opens that cut for editing).
    fn grab_at(&mut self, at: Point) -> Option<Grab> {
        let ui = self.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
        let near = |p: V3, me: &Self| me.screen(p).is_some_and(|s| (s - at).length() < GRAB * ui);
        if let Some(a) = self.active {
            if let Some(i) = self.draft.iter().position(|d| near(a.plane.to_3d(*d, a.offset), self)) {
                return Some(Grab::Draft(i));
            }
        }
        // a joint: selected, and moved when dragged
        if self.joint.is_none() {
            if let Some(i) = self.rig.joints.iter().position(|j| j.position().is_some_and(|(p, _)| near(p, self))) {
                self.selected = Some(self.rig.joints[i].name.clone());
                self.message = format!("joint {} selected: drag to move it{}", self.rig.joints[i].name, if self.rig.joints[i].proposed { ", D dismisses it" } else { "" });
                self.remember();
                return Some(Grab::Joint(i));
            }
        }
        // a corner of any section of a cut, seen from wherever the camera is
        let find = |cut: &Cut, me: &Self| -> Option<(usize, usize, usize)> {
            for (vi, v) in cut.views.iter().enumerate() {
                for (si, s) in v.sections.iter().enumerate() {
                    if let Some(pi) = s.contour.iter().position(|q| near(v.plane.to_3d(*q, s.offset), me)) {
                        return Some((vi, si, pi));
                    }
                }
            }
            None
        };
        // a section's outline: the region (or, with Shift, the whole cut) and where on it
        let outline = |cut: &Cut, me: &Self| -> Option<(usize, usize, V3)> {
            for (vi, v) in cut.views.iter().enumerate() {
                for (si, s) in v.sections.iter().enumerate() {
                    let n = s.contour.len();
                    for k in 0..n {
                        let (p, q) = (v.plane.to_3d(s.contour[k], s.offset), v.plane.to_3d(s.contour[(k + 1) % n], s.offset));
                        let (Some(a), Some(b)) = (me.screen(p), me.screen(q)) else { continue };
                        let ab = b - a;
                        let t = (((at - a).x * ab.x + (at - a).y * ab.y) / (ab.x * ab.x + ab.y * ab.y).max(1e-6)).clamp(0.0, 1.0);
                        if (a + ab * t - at).length() < GRAB * ui {
                            return Some((vi, si, std::array::from_fn(|c| p[c] + (q[c] - p[c]) * t)));
                        }
                    }
                }
            }
            None
        };
        let whole = self.modifiers.shift_key();
        if let Some(found) = self.pending.as_ref().and_then(|c| find(c, self)) {
            self.keep_step();
            return Some(Grab::Section(found.0, found.1, found.2));
        }
        if let Some((vi, si, p)) = self.pending.as_ref().and_then(|c| outline(c, self)) {
            self.keep_step();
            self.message = if whole { "moving the whole cut".into() } else { "moving this region (Shift: the whole cut)".into() };
            return Some(Grab::Outline { view: vi, section: si, whole, last: p });
        }
        if self.pending.is_some() {
            return None;
        }
        if let Some((i, (vi, si, pi))) = (0..self.rig.cuts.len()).find_map(|i| find(&self.rig.cuts[i], self).map(|f| (i, f))) {
            self.keep_step();
            self.open_cut(i);
            return Some(Grab::Section(vi, si, pi));
        }
        let (i, (vi, si, p)) = (0..self.rig.cuts.len()).find_map(|i| outline(&self.rig.cuts[i], self).map(|f| (i, f)))?;
        self.keep_step();
        self.open_cut(i);
        self.message = if whole { "moving the whole cut".into() } else { "moving this region (Shift: the whole cut)".into() };
        Some(Grab::Outline { view: vi, section: si, whole, last: p })
    }

    /// Saves the unaccepted work before a change, for undo.
    fn keep_step(&mut self) {
        self.steps.push((self.pending.clone(), self.draft.clone(), self.joint.clone()));
    }

    fn undo(&mut self, back: bool) {
        // unaccepted work first, step by step (a dragged dot, a moved region, a placed dot)
        if back {
            if let Some((cut, draft, joint)) = self.steps.pop() {
                if cut.is_none() {
                    self.editing = None;
                }
                self.pending = cut;
                self.draft = draft;
                self.joint = joint;
                self.message = "step undone".into();
                return;
            }
        }
        let (from, to) = if back { (&mut self.history, &mut self.future) } else { (&mut self.future, &mut self.history) };
        match from.pop() {
            Some(state) => {
                to.push(std::mem::replace(&mut self.rig, state));
                self.message = if back { "undone".into() } else { "redone".into() };
                self.save();
            }
            None => self.message = "nothing to undo".into(),
        }
    }

    /// Rebuilds the model with the accepted cuts in the background.
    fn apply(&mut self) {
        if self.applying.is_some() {
            self.message = "already rebuilding".into();
            return;
        }
        let Some(source) = self.rig.source.clone() else {
            self.message = "no source video: start with --source video.mp4 --options \"...\"".into();
            return;
        };
        let stem = self.model_path.strip_suffix(".glb").unwrap_or(&self.model_path);
        let (full, shown) = match stem.strip_suffix("_low") {
            Some(s) => (format!("{s}_cut.glb"), format!("{s}_cut_low.glb")),
            None => (format!("{stem}_cut.glb"), format!("{stem}_cut.glb")),
        };
        let tool = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("fframes-vector"))).unwrap_or_else(|| "fframes-vector".into());
        let rig_file = self.rig_file.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let out = std::process::Command::new(&tool)
                .arg("depth")
                .arg(&source.video)
                .arg(&full)
                .args(&source.options)
                .arg("--cuts")
                .arg(&rig_file)
                .output();
            let result = match out {
                Ok(o) if o.status.success() => {
                    let text = String::from_utf8_lossy(&o.stderr).to_string();
                    let removed = text.lines().find(|l| l.starts_with("cuts took out")).unwrap_or("").to_string();
                    Ok(format!("{shown}\n{removed}"))
                }
                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("failed").to_string()),
                Err(e) => Err(format!("{}: {e}", tool.display())),
            };
            let _ = tx.send(result);
        });
        self.applying = Some(rx);
        self.message = "rebuilding the model with the cuts (about a minute)".into();
    }

    fn check_apply(&mut self) {
        let Some(rx) = &self.applying else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.applying = None;
        match result {
            Ok(text) => {
                let mut lines = text.lines();
                let shown = lines.next().unwrap_or_default().to_string();
                let removed = lines.next().unwrap_or_default().to_string();
                match load_figure(&shown) {
                    Ok(f) => {
                        self.scenes.1 = Some(room_scene(&f, self.lo, self.centre, self.room, self.rig.front_yaw));
                        self.rebuilt = Some(f);
                        self.showing_rebuilt = true;
                        self.posed = None;
                        self.message = format!("rebuilt: {shown} ({removed}); T switches to the original");
                    }
                    Err(e) => self.message = e,
                }
            }
            Err(e) => self.message = format!("rebuild failed: {e}"),
        }
        self.redraw();
    }

    fn click(&mut self, at: PhysicalPosition<f64>) {
        let ui = self.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
        let _ = ui;
        let (px, py) = (at.x as f32, at.y as f32);
        if let Some(&(_, action)) = self.buttons.iter().find(|(r, _)| px >= r.left && px <= r.right && py >= r.top && py <= r.bottom) {
            self.act(action);
            return;
        }
        let Some(a) = self.active else {
            self.message = "choose a plane first (icons top left)".into();
            return;
        };
        let Some(q) = self.plane_point(at) else {
            self.message = "the plane is edge-on here: turn the camera".into();
            return;
        };
        self.keep_step();
        if let Some(j) = &mut self.joint {
            j.observations.push(Observation { plane: a.plane, offset: a.offset, point: q });
            self.message = format!("joint {}: placed on the {} plane ({} placements)", j.name, a.preset.name(), j.observations.len());
        } else {
            self.draft.push(q);
            self.message = format!("{} dots; Enter closes the contour", self.draft.len());
        }
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Plane(p) => self.choose_plane(p),
            Action::Protect => {
                let n = self.rig.cuts.len() + 1;
                let cut = self.pending.get_or_insert_with(|| Cut { name: format!("cut {n}"), ..Cut::default() });
                cut.protect = !cut.protect;
                if cut.protect && !cut.name.starts_with("protected") {
                    cut.name = format!("protected {n}");
                } else if !cut.protect {
                    cut.name = format!("cut {n}");
                }
                self.message = format!("{}: {}", cut.name, if cut.protect { "protected region, no cut removes anything inside" } else { "removes its volume" });
            }
            Action::Joint => {
                self.typing = Some(String::new());
                self.message = "joint name (name:parent for a parent), Enter".into();
            }
            Action::Accept => self.accept(),
            Action::Discard => self.discard(),
            Action::Undo => self.undo(true),
            Action::Apply => self.apply(),
            Action::Show => {
                if self.rebuilt.is_some() {
                    self.showing_rebuilt = !self.showing_rebuilt;
                    self.posed = None;
                    self.message = if self.showing_rebuilt { "the rebuilt model".into() } else { "the original model".into() };
                } else {
                    self.message = "not rebuilt yet (Apply)".into();
                }
            }
        }
    }

    /// Turns the arm about the shoulder by `side` degrees out to the side and `forward`
    /// degrees forward (both 0 with `reset`): the joints `shoulder` and the one whose parent it
    /// is (`elbow:shoulder`), the weights found on the figure shown.
    fn turn_arm(&mut self, side: f32, forward: f32, reset: bool) {
        if self.posed.is_none() {
            // the selected shoulder, else the first one; its elbow is the joint it is parent of
            let is_shoulder = |j: &&Joint| j.name.starts_with("shoulder");
            let shoulder = self
                .selected
                .as_ref()
                .and_then(|n| self.rig.joints.iter().filter(is_shoulder).find(|j| &j.name == n))
                .or_else(|| self.rig.joints.iter().find(is_shoulder));
            let elbow = shoulder.and_then(|s| self.rig.joints.iter().find(|j| j.parent.as_deref() == Some(s.name.as_str())));
            let (Some((s, _)), Some((e, _))) = (shoulder.and_then(Joint::position), elbow.and_then(Joint::position)) else {
                self.message = "place and accept the joints shoulder and elbow (named elbow:shoulder) first".into();
                return;
            };
            let figure = self.shown();
            match limb_weights(&figure.positions, &figure.indices, s, e, 0.04) {
                Some((weights, level)) => {
                    let moving = weights.iter().filter(|&&w| w > 0.0).count();
                    self.message = format!("arm free from {:.0} mm below the shoulder; {moving} of {} vertices follow", level * 1000.0, weights.len());
                    self.posed = Some(Posed { weights, shoulder: s, elbow: e, side: 0.0, forward: 0.0, scene: None });
                }
                None => {
                    self.message = "the arm is not free of the body anywhere below the shoulder: cut the bridge first (and Apply)".into();
                    return;
                }
            }
        }
        if let Some(p) = &mut self.posed {
            if reset {
                p.side = 0.0;
                p.forward = 0.0;
            } else {
                p.side = (p.side + side).clamp(-30.0, 170.0);
                p.forward = (p.forward + forward).clamp(-60.0, 170.0);
            }
        }
        let Some(p) = &self.posed else { return };
        let (side_axis, _, front) = axes(self.rig.front_yaw);
        // out to the side: about the front axis, the way that takes the elbow away from the
        // body's middle; forward: about the side axis, the way that takes it to the front
        let centre = self.centre.to_array();
        let away = |r: &[[f32; 3]; 3]| {
            let d: V3 = std::array::from_fn(|k| p.elbow[k] - p.shoulder[k]);
            let t: V3 = std::array::from_fn(|k| p.shoulder[k] + r[k][0] * d[0] + r[k][1] * d[1] + r[k][2] * d[2]);
            ((t[0] - centre[0]).powi(2) + (t[2] - centre[2]).powi(2), t[0] * front[0] + t[2] * front[2])
        };
        let out_sign = if away(&rotation(front, 0.3)).0 >= away(&rotation(front, -0.3)).0 { 1.0 } else { -1.0 };
        let fwd_sign = if away(&rotation(side_axis, 0.3)).1 >= away(&rotation(side_axis, -0.3)).1 { 1.0 } else { -1.0 };
        let rot = mul(&rotation(side_axis, fwd_sign * p.forward.to_radians()), &rotation(front, out_sign * p.side.to_radians()));
        let (side_deg, forward_deg) = (p.side, p.forward);
        let figure = self.shown();
        let points = pose(&figure.positions, &p.weights, p.shoulder, &rot);
        // the figure's own scene with its meshes moved
        let mut tree = figure.model.scene.clone();
        let meshes: Vec<_> = tree.nodes().filter(|(_, n)| n.as_mesh().is_some()).map(|(id, _)| id).collect();
        let mut at = 0;
        for id in meshes {
            let to_local = tree.world_matrix(id).inverse();
            if let Some(mesh) = tree[id].mesh_mut() {
                let g = &mesh.geometry;
                let n = g.positions.len();
                let moved: Vec<Vec3> = points[at..at + n].iter().map(|q| to_local.transform_point3(Vec3::from_array(*q))).collect();
                mesh.geometry = std::sync::Arc::new(Geometry::new(moved, Vec::new(), g.uvs.clone(), g.indices.clone()));
                at += n;
            }
        }
        let posed = Figure { model: Model { scene: tree, animations: Vec::new() }, positions: Vec::new(), indices: Vec::new() };
        let scene = room_scene(&posed, self.lo, self.centre, self.room, self.rig.front_yaw);
        if let Some(p) = &mut self.posed {
            p.scene = Some(scene);
        }
        if !reset {
            self.message = format!("arm {side_deg:.0}° out to the side, {forward_deg:.0}° forward (, . side   ; ' forward   0 rest)");
        } else {
            self.message = "arm at rest".into();
        }
    }

    fn draw(&mut self) {
        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        let (rw, rh) = (((size.width as f32 * QUALITY) as u32).max(16), ((size.height as f32 * QUALITY) as u32).max(16));
        if self.renderer.as_ref().is_none_or(|(w, h, _)| (*w, *h) != (rw, rh)) {
            let r = Renderer::new(rw, rh).gpu();
            if !r.uses_gpu() {
                eprintln!("no GPU found, rendering on the CPU (slow)");
            }
            self.renderer = Some((rw, rh, r));
        }
        let mut scene = match (self.posed.as_ref().and_then(|p| p.scene.as_ref()), &self.scenes.1, self.showing_rebuilt) {
            (Some(s), _, _) => s.clone(),
            (None, Some(s), true) => s.clone(),
            _ => self.scenes.0.clone(),
        };
        // the active plane as a pane of glass through the figure
        if let Some(a) = self.active {
            let reach = (self.hi - self.lo).length() * 0.75;
            let (n, u, v) = (Vec3::from_array(a.plane.normal), Vec3::from_array(a.plane.u), Vec3::from_array(a.plane.v));
            let c = self.centre - n * n.dot(self.centre) + n * a.offset;
            let colour = if self.pending.as_ref().is_some_and(|c| c.protect) { 0x50_d0_80 } else { 0x58_a8_ff };
            scene.add(
                Node::mesh(Geometry::plane(reach * 2.0, reach * 2.0), Material::unlit(Rgb::hex(colour)).opacity(0.18).transparent().double_sided())
                    .with_position(c)
                    .with_rotation(Quat::from_mat3(&Mat3::from_cols(u, v, n)))
                    .shadows(false, false),
            );
        }
        let camera = self.camera();
        let Some((_, _, renderer)) = self.renderer.as_ref() else { return };
        let pixels = renderer.render_pixels(&scene, &camera);
        let info = ImageInfo::new((pixels.width as i32, pixels.height as i32), ColorType::RGBA8888, AlphaType::Premul, None);
        let image = skia_safe::images::raster_from_data(&info, Data::new_copy(&pixels.rgba), pixels.width as usize * 4);

        // overlay: everything as screen lines and dots
        let rig = self.with_pending();
        let removal = fframes_rig::Removal::new(&rig);
        let mut lines: Vec<(Point, Point, Color, f32)> = Vec::new();
        let mut dots: Vec<(Point, Color, f32)> = Vec::new();
        let figure = self.shown();
        if let Some(a) = self.active {
            for (p, q) in section_segments(&figure.positions, &figure.indices, &a.plane, a.offset) {
                let mid: V3 = std::array::from_fn(|k| f32::midpoint(p[k], q[k]));
                let colour = if removal.removes(mid) { Color::from_rgb(255, 70, 60) } else { Color::from_rgb(245, 245, 245) };
                if let (Some(s), Some(t)) = (self.screen(p), self.screen(q)) {
                    lines.push((s, t, colour, 2.0));
                }
            }
            for (i, d) in self.draft.iter().enumerate() {
                let p = a.plane.to_3d(*d, a.offset);
                if let Some(s) = self.screen(p) {
                    dots.push((s, Color::from_rgb(255, 210, 60), 5.0));
                    let next = self.draft.get(i + 1).or(if self.draft.len() > 2 { self.draft.first() } else { None });
                    if let Some(t) = next.and_then(|n| self.screen(a.plane.to_3d(*n, a.offset))) {
                        lines.push((s, t, Color::from_rgb(255, 210, 60), 1.5));
                    }
                }
            }
        }
        // cuts: their sections, and outlines through the volume between them
        let pending_name = self.pending.as_ref().map(|c| c.name.clone());
        for cut in &rig.cuts {
            // proposed magenta, being edited orange, protected green, accepted blue
            let colour = match (cut.protect, Some(&cut.name) == pending_name.as_ref(), cut.proposed) {
                (_, true, _) => Color::from_rgb(255, 150, 50),
                (_, false, true) => Color::from_rgb(235, 80, 225),
                (true, false, false) => Color::from_rgb(80, 210, 130),
                (false, false, false) => Color::from_rgb(90, 170, 255),
            };
            for view in &cut.views {
                let Some((lo, hi)) = view.range() else { continue };
                // every section's corners as dots, to drag from any view
                for s in &view.sections {
                    for q in &s.contour {
                        if let Some(p) = self.screen(view.plane.to_3d(*q, s.offset)) {
                            dots.push((p, colour, 5.0));
                        }
                    }
                }
                if view.corresponding() {
                    // one box: each level's outline, the edges from corner to corner between
                    // levels, and where the active plane cuts it
                    let mut seg = |p: V3, q: V3, w: f32| {
                        if let (Some(s), Some(e)) = (self.screen(p), self.screen(q)) {
                            lines.push((s, e, colour, w));
                        }
                    };
                    for (l, s) in view.sections.iter().enumerate() {
                        let n = s.contour.len();
                        for k in 0..n {
                            seg(view.plane.to_3d(s.contour[k], s.offset), view.plane.to_3d(s.contour[(k + 1) % n], s.offset), 2.0);
                            if let Some(up) = view.sections.get(l + 1) {
                                seg(view.plane.to_3d(s.contour[k], s.offset), view.plane.to_3d(up.contour[k], up.offset), 2.0);
                            }
                        }
                    }
                    if let Some(a) = self.active {
                        let (pos, ind) = view.surface();
                        for (p, q) in section_segments(&pos, &ind, &a.plane, a.offset) {
                            seg(p, q, 3.5);
                        }
                    }
                    continue;
                }
                let mut offsets: Vec<f32> = view.sections.iter().map(|s| s.offset).collect();
                offsets.extend((0..=6).map(|k| lo + (hi - lo) * k as f32 / 6.0));
                for (n, t) in offsets.iter().enumerate() {
                    let Some(c) = view.contour_at(*t) else { continue };
                    let width = if n < view.sections.len() { 2.0 } else { 0.8 };
                    for i in 0..c.len() {
                        let (p, q) = (view.plane.to_3d(c[i], *t), view.plane.to_3d(c[(i + 1) % c.len()], *t));
                        if let (Some(s), Some(e)) = (self.screen(p), self.screen(q)) {
                            lines.push((s, e, colour, width));
                        }
                    }
                }
            }
        }
        // joints and the bones between them
        // by side: the figure's left blue, its right orange, the middle yellow; a proposed joint
        // is a ring, an accepted one filled, the one being placed white; the selected one
        // ringed white
        for j in &rig.joints {
            let pending = self.joint.as_ref().is_some_and(|p| p.name == j.name);
            let Some((p, _)) = j.position() else { continue };
            let colour = if pending {
                Color::WHITE
            } else if j.name.ends_with("_l") {
                Color::from_rgb(80, 150, 255)
            } else if j.name.ends_with("_r") {
                Color::from_rgb(255, 130, 50)
            } else {
                Color::from_rgb(245, 215, 60)
            };
            if let Some(s) = self.screen(p) {
                if self.selected.as_ref() == Some(&j.name) {
                    dots.push((s, Color::WHITE, 9.0));
                }
                dots.push((s, colour, 6.0));
                if j.proposed && !pending {
                    dots.push((s, Color::from_rgb(25, 25, 30), 3.0));
                }
                if let Some(parent) = j.parent.as_ref().and_then(|n| rig.joints.iter().find(|q| &q.name == n)).and_then(Joint::position) {
                    if let Some(t) = self.screen(parent.0) {
                        lines.push((s, t, colour, 2.5));
                    }
                }
            }
        }

        let ui = window.scale_factor() as f32;
        let font = Font::from_typeface(&self.typeface, 13.0 * ui);
        let alerts = self.alerts();
        let status = if let Some(t) = &self.typing {
            format!("joint name: {t}_")
        } else {
            let plane = self.active.map_or_else(|| "no plane".to_string(), |a| format!("{} plane at {:.3} m", a.preset.name(), a.offset));
            let front = if self.rig.front_confirmed { format!("front {:.0}°", self.rig.front_yaw) } else { "front not confirmed".into() };
            format!(
                "{plane}   {} cuts, {} joints   {front}   {}{}",
                self.rig.cuts.len(),
                self.rig.joints.len(),
                if self.showing_rebuilt { "showing the rebuilt model   " } else { "" },
                self.message
            )
        };
        let keys = "click: dot / joint   drag any dot: move   Enter: close contour   Up/Down or ⌥+drag: move plane   drag outline: region (Shift: whole cut)   ⌥+click box dot: remove, edge: add   E: edit cut   N: next section   Delete or Cmd+Z: undo step   Shift+Delete: remove section   [ ]: ends   P: protect   A: accept   D: discard   J: joint   X: apply   T: original/rebuilt   , . ; ' 0: arm   F: confirm front   B: turn front   Cmd+Z: undo   Q: quit";
        let labels: [(&str, Action); 11] = [
            ("Transverse", Action::Plane(Preset::Transverse)),
            ("Coronal", Action::Plane(Preset::Coronal)),
            ("Sagittal", Action::Plane(Preset::Sagittal)),
            ("Protect", Action::Protect),
            ("Joint", Action::Joint),
            ("Accept", Action::Accept),
            ("Discard", Action::Discard),
            ("Undo", Action::Undo),
            ("Apply", Action::Apply),
            ("Original/rebuilt", Action::Show),
            ("", Action::Show),
        ];
        let active_preset = self.active.map(|a| a.preset);
        let protect = self.pending.as_ref().is_some_and(|c| c.protect);
        let mut buttons = Vec::new();
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
            let mut pen = Paint::default();
            pen.set_anti_alias(true);
            pen.set_style(PaintStyle::Stroke);
            for (a, b, c, width) in &lines {
                pen.set_color(*c);
                pen.set_stroke_width(width * ui);
                canvas.draw_line(*a, *b, &pen);
            }
            let mut fill = Paint::default();
            fill.set_anti_alias(true);
            for (p, c, r) in &dots {
                fill.set_color(Color::BLACK);
                canvas.draw_circle(*p, (r + 1.5) * ui, &fill);
                fill.set_color(*c);
                canvas.draw_circle(*p, r * ui, &fill);
            }
            // the icons
            let (mut x, y) = (10.0 * ui, 10.0 * ui);
            for (label, action) in labels.iter().filter(|(l, _)| !l.is_empty()) {
                let tw = font.measure_str(label, None).0;
                let r = Rect::from_xywh(x, y, tw + 16.0 * ui, 26.0 * ui);
                let on = match action {
                    Action::Plane(p) => active_preset == Some(*p),
                    Action::Protect => protect,
                    _ => false,
                };
                fill.set_color(if on { Color::from_rgb(70, 130, 220) } else { Color::from_argb(200, 30, 32, 38) });
                canvas.draw_round_rect(r, 5.0 * ui, 5.0 * ui, &fill);
                fill.set_color(Color::from_rgb(235, 235, 235));
                canvas.draw_str(label, (x + 8.0 * ui, y + 17.5 * ui), &font, &fill);
                buttons.push((r, *action));
                x += tw + 22.0 * ui;
            }
            // what is not finished
            if !alerts.is_empty() {
                let bw = 520.0 * ui;
                let bh = (alerts.len() as f32 * 17.0 + 30.0) * ui;
                let r = Rect::from_xywh(w - bw - 10.0 * ui, 46.0 * ui, bw, bh);
                fill.set_color(Color::from_argb(225, 70, 52, 14));
                canvas.draw_round_rect(r, 6.0 * ui, 6.0 * ui, &fill);
                fill.set_color(Color::from_rgb(255, 205, 90));
                canvas.draw_str("Not finished", (r.left + 10.0 * ui, r.top + 18.0 * ui), &font, &fill);
                fill.set_color(Color::from_rgb(250, 235, 200));
                for (i, a) in alerts.iter().enumerate() {
                    let text: String = a.chars().take(80).collect();
                    canvas.draw_str(&text, (r.left + 10.0 * ui, r.top + (36.0 + i as f32 * 17.0) * ui), &font, &fill);
                }
            }
            fill.set_color(Color::from_argb(180, 0, 0, 0));
            canvas.draw_rect(Rect::from_xywh(0.0, h - 44.0 * ui, w, 44.0 * ui), &fill);
            fill.set_color(Color::from_rgb(230, 230, 230));
            canvas.draw_str(&status, (10.0 * ui, h - 26.0 * ui), &font, &fill);
            fill.set_color(Color::from_rgb(150, 150, 150));
            canvas.draw_str(keys, (10.0 * ui, h - 9.0 * ui), &font, &fill);
        });
        self.buttons = buttons;
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Plane movement for a key: Shift finer, Alt coarser.
    fn step(&self) -> f32 {
        if self.modifiers.shift_key() {
            0.001
        } else if self.modifiers.alt_key() {
            0.02
        } else {
            0.005
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, logical: &Key, code: Option<KeyCode>) {
        // typing a joint name
        if let Some(t) = &mut self.typing {
            match logical {
                Key::Named(NamedKey::Enter) => {
                    let text = std::mem::take(t);
                    self.typing = None;
                    let (name, parent) = match text.split_once(':') {
                        Some((n, p)) => (n.trim().to_string(), Some(p.trim().to_string()).filter(|p| !p.is_empty())),
                        None => (text.trim().to_string(), None),
                    };
                    if name.is_empty() {
                        self.message = "no name: joint cancelled".into();
                    } else {
                        // an existing joint is continued with its placements
                        let existing = self.rig.joints.iter().find(|j| j.name == name).cloned();
                        let joint = existing.unwrap_or(Joint { name: name.clone(), ..Joint::default() });
                        self.joint = Some(Joint { parent: parent.or(joint.parent.clone()), ..joint });
                        self.message = format!("joint {name}: click it on a plane, then on a second plane across it");
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    self.typing = None;
                    self.message = "joint cancelled".into();
                }
                Key::Named(NamedKey::Backspace) => {
                    t.pop();
                }
                Key::Named(NamedKey::Space) => t.push(' '),
                Key::Character(c) => t.push_str(c),
                _ => {}
            }
            self.redraw();
            return;
        }
        let command = self.modifiers.super_key() || self.modifiers.control_key();
        let Some(code) = code else { return };
        if code != KeyCode::KeyQ {
            self.quit_armed = false;
        }
        match code {
            KeyCode::KeyQ => {
                if (self.pending.is_some() || self.joint.is_some() || !self.draft.is_empty()) && !self.quit_armed {
                    self.quit_armed = true;
                    self.message = "not accepted work would be lost: Q again to quit, A to accept".into();
                } else {
                    for task in self.rig.open_tasks() {
                        eprintln!("open: {task}");
                    }
                    event_loop.exit();
                }
            }
            KeyCode::Escape => {
                if !self.draft.is_empty() {
                    self.draft.clear();
                    self.message = "contour dropped".into();
                } else if self.active.take().is_some() {
                    self.message = "plane put away".into();
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => self.close_contour(),
            KeyCode::Backspace if command => self.undo(true),
            KeyCode::Backspace => {
                if self.draft.pop().is_some() {
                    self.message = format!("{} dots", self.draft.len());
                } else if let Some(j) = &mut self.joint {
                    j.observations.pop();
                    self.message = format!("joint {}: {} placements", j.name, j.observations.len());
                }
            }
            KeyCode::ArrowUp => {
                let s = self.step();
                self.move_plane(s);
            }
            KeyCode::ArrowDown => {
                let s = self.step();
                self.move_plane(-s);
            }
            KeyCode::BracketLeft | KeyCode::BracketRight => {
                let longer = code == KeyCode::BracketRight;
                let upper = self.modifiers.shift_key();
                let active = self.active;
                if let (Some(cut), Some(a)) = (&mut self.pending, active) {
                    if let Some(v) = cut.views.iter_mut().find(|v| v.plane.parallel_to(&a.plane)) {
                        let end = if upper { &mut v.after } else { &mut v.before };
                        *end = (*end + if longer { 0.002 } else { -0.002 }).max(0.0);
                        self.message = format!("reach below the first section {:.0} mm, above the last {:.0} mm", v.before * 1000.0, v.after * 1000.0);
                    }
                } else {
                    self.message = "draw a section of the cut first".into();
                }
            }
            KeyCode::KeyA => self.accept(),
            KeyCode::KeyD => self.discard(),
            KeyCode::KeyP => self.act(Action::Protect),
            KeyCode::KeyJ => self.act(Action::Joint),
            KeyCode::KeyX => self.apply(),
            KeyCode::KeyT => self.act(Action::Show),
            KeyCode::KeyZ if command => {
                let redo = self.modifiers.shift_key();
                self.undo(!redo);
            }
            KeyCode::KeyF => {
                self.remember();
                self.rig.front_confirmed = true;
                self.save();
                self.message = format!("front confirmed at {:.0}°", self.rig.front_yaw);
            }
            KeyCode::KeyB => {
                // the other way round, or (Shift) a quarter turn when the width was misread
                let by = if self.modifiers.shift_key() { 90.0 } else { 180.0 };
                self.rig.front_yaw = (self.rig.front_yaw + by).rem_euclid(360.0);
                self.rig.front_confirmed = false;
                self.scenes.0 = room_scene(&self.figure, self.lo, self.centre, self.room, self.rig.front_yaw);
                if let Some(r) = &self.rebuilt {
                    self.scenes.1 = Some(room_scene(r, self.lo, self.centre, self.room, self.rig.front_yaw));
                }
                self.posed = None;
                if let Some(a) = &mut self.active {
                    a.plane = a.preset.plane(self.rig.front_yaw);
                }
                self.azimuth = self.rig.front_yaw;
                self.message = format!("front turned to {:.0}°: F confirms", self.rig.front_yaw);
            }
            KeyCode::KeyR => {
                self.target = self.centre;
                self.distance = self.height * 2.2;
                self.azimuth = self.rig.front_yaw;
                self.elevation = 8.0;
            }
            KeyCode::KeyE => self.edit_cut(),
            KeyCode::KeyN => {
                let back = self.modifiers.shift_key();
                self.jump_section(back);
            }
            // Delete undoes the last step; Shift+Delete removes the section at the plane
            KeyCode::Delete if self.modifiers.shift_key() => self.delete_section(),
            KeyCode::Delete => self.undo(true),
            KeyCode::Period => self.turn_arm(5.0, 0.0, false),
            KeyCode::Comma => self.turn_arm(-5.0, 0.0, false),
            KeyCode::Semicolon => self.turn_arm(0.0, 5.0, false),
            KeyCode::Quote => self.turn_arm(0.0, -5.0, false),
            KeyCode::Digit0 => self.turn_arm(0.0, 0.0, true),
            KeyCode::Digit1 => self.choose_plane(Preset::Transverse),
            KeyCode::Digit2 => self.choose_plane(Preset::Coronal),
            KeyCode::Digit3 => self.choose_plane(Preset::Sagittal),
            _ => return,
        }
        self.redraw();
    }
}

impl ApplicationHandler for Editor {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("fframes-rig-editor").with_inner_size(LogicalSize::new(1200, 900));
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
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                self.draw();
                if self.error.is_some() {
                    event_loop.exit();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::Focused(true) => self.focused_at = Some(std::time::Instant::now()),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let code = match event.physical_key {
                        PhysicalKey::Code(c) => Some(c),
                        PhysicalKey::Unidentified(_) => None,
                    };
                    self.key(event_loop, &event.logical_key, code);
                    self.autosave();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((down, last)) = self.pressed {
                    if let Some(g) = self.grabbed {
                        match g {
                            Grab::Draft(i) => {
                                // across the screen, then onto the plane (never along the
                                // direction towards the camera, where a slanted plane would
                                // magnify the pointer's movement)
                                if let Some(a) = self.active {
                                    let at = a.plane.to_3d(self.draft[i], a.offset);
                                    if let Some(p) = self.screen_drag(position, at) {
                                        self.draft[i] = a.plane.to_2d(p);
                                    }
                                }
                            }
                            Grab::Outline { view, section, whole, last } => {
                                // across the screen at the grabbed point's depth
                                let camera = self.camera();
                                let plane = Plane { normal: camera.forward().to_array(), u: camera.right().to_array(), v: camera.up_vector().to_array() };
                                let hit = self.ray(position).and_then(|(o, d)| plane.hit(plane.offset_of(last), o, d));
                                if let (Some((now, _)), Some(cut)) = (hit, &mut self.pending) {
                                    let d = Vec3::from_array(now) - Vec3::from_array(last);
                                    for (vi, v) in cut.views.iter_mut().enumerate() {
                                        let (n, u, w) = (Vec3::from_array(v.plane.normal), Vec3::from_array(v.plane.u), Vec3::from_array(v.plane.v));
                                        for (si, s) in v.sections.iter_mut().enumerate() {
                                            if whole || (vi == view && si == section) {
                                                s.offset += d.dot(n);
                                                for q in &mut s.contour {
                                                    q[0] += d.dot(u);
                                                    q[1] += d.dot(w);
                                                }
                                            }
                                        }
                                    }
                                    self.grabbed = Some(Grab::Outline { view, section, whole, last: now });
                                }
                            }
                            Grab::Joint(i) => {
                                // across the screen, at the joint's depth
                                let camera = self.camera();
                                let ray = self.ray(position);
                                if let (Some((o, d)), Some((p, _))) = (ray, self.rig.joints.get(i).and_then(Joint::position)) {
                                    let n = camera.forward().to_array();
                                    let plane = Plane { normal: n, u: camera.right().to_array(), v: camera.up_vector().to_array() };
                                    if let Some((q, _)) = plane.hit(plane.offset_of(p), o, d) {
                                        let yaw = self.rig.front_yaw;
                                        self.rig.joints[i].set_at(q, yaw);
                                        self.posed = None;
                                        self.message = format!("joint {} at x {:.3} y {:.3} z {:.3}", self.rig.joints[i].name, q[0], q[1], q[2]);
                                    }
                                }
                            }
                            Grab::Section(vi, si, pi) => {
                                // across the screen, then within the active plane through the
                                // dot (no plane: kept on its level); where that plane crosses
                                // the levels (sagittal, coronal) the height moves the box's
                                // whole level
                                let active = self.active;
                                let at = self.pending.as_ref().map(|c| {
                                    let v = &c.views[vi];
                                    v.plane.to_3d(v.sections[si].contour[pi], v.sections[si].offset)
                                });
                                let moved = at.and_then(|at| self.screen_drag(position, at).map(|p| (at, p)));
                                if let (Some((at, mut p)), Some(cut)) = (moved, &mut self.pending) {
                                    if let Some(a) = active {
                                        let n = Vec3::from_array(a.plane.normal);
                                        let off = (Vec3::from_array(p) - Vec3::from_array(at)).dot(n);
                                        p = (Vec3::from_array(p) - n * off).to_array();
                                    }
                                    let v = &mut cut.views[vi];
                                    let moves_level = v.corresponding() && active.is_some_and(|a| !a.plane.parallel_to(&v.plane));
                                    let s = &mut v.sections[si];
                                    s.contour[pi] = v.plane.to_2d(p);
                                    if moves_level {
                                        s.offset = v.plane.offset_of(p);
                                    }
                                }
                            }
                        }
                    } else {
                        let (dx, dy) = ((position.x - last.x) as f32, (position.y - last.y) as f32);
                        if let (true, Some(a)) = (self.modifiers.alt_key(), self.active) {
                            // Option+drag: the plane follows the pointer along its normal (the
                            // normal's direction on the screen, 10 cm of it measured there)
                            let at = a.plane.to_3d(a.plane.to_2d(self.centre.to_array()), a.offset);
                            let ahead: V3 = std::array::from_fn(|k| at[k] + a.plane.normal[k] * 0.1);
                            if let (Some(s), Some(t)) = (self.screen(at), self.screen(ahead)) {
                                let (nx, ny) = (t.x - s.x, t.y - s.y);
                                let len2 = nx * nx + ny * ny;
                                if len2 > 4.0 {
                                    self.move_plane((dx * nx + dy * ny) / len2 * 0.1);
                                } else {
                                    self.message = "the plane faces the camera: turn the camera to drag it, or use Up/Down".into();
                                }
                            }
                        } else if self.modifiers.shift_key() {
                            self.target.y += dy * self.distance * 0.0015;
                        } else {
                            self.azimuth -= dx * 0.3;
                            self.elevation = (self.elevation + dy * 0.3).clamp(-85.0, 85.0);
                        }
                    }
                    self.pressed = Some((down, position));
                    self.redraw();
                }
                self.cursor = position;
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => {
                    self.pressed = Some((self.cursor, self.cursor));
                    // the click that brings the window to the front places nothing
                    self.activation_click = self.focused_at.is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(400));
                    // a press on a dot (of the open contour, or of a cut's section on the
                    // plane) grabs it
                    let at = Point::new(self.cursor.x as f32, self.cursor.y as f32);
                    // Option+click adds or removes a box's dot instead of grabbing
                    self.grabbed = if self.activation_click || self.modifiers.alt_key() { None } else { self.grab_at(at) };
                }
                ElementState::Released => {
                    let grabbed = self.grabbed.take();
                    if let Some((down, _)) = self.pressed.take() {
                        let moved = ((self.cursor.x - down.x).powi(2) + (self.cursor.y - down.y).powi(2)).sqrt();
                        let activation = std::mem::take(&mut self.activation_click);
                        match grabbed {
                            // a click on the first dot of an open contour closes it
                            Some(Grab::Draft(0)) if moved < CLICK && self.draft.len() >= 3 => self.close_contour(),
                            // a moved joint is kept at once (Cmd+Z takes it back)
                            Some(Grab::Joint(_)) if moved >= CLICK => self.save(),
                            // a moved region may now lie past its neighbour: keep the order
                            Some(Grab::Outline { .. } | Grab::Section(..)) => {
                                if let Some(cut) = &mut self.pending {
                                    cut.views.iter_mut().for_each(View::sort);
                                }
                            }
                            Some(Grab::Joint(_)) => {
                                self.history.pop();
                            }
                            None if moved < CLICK && !activation && self.modifiers.alt_key() => self.box_dot(self.cursor),
                            None if moved < CLICK && !activation => self.click(self.cursor),
                            _ => {}
                        }
                        self.autosave();
                        self.redraw();
                    }
                }
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 20.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if self.modifiers.alt_key() && self.active.is_some() {
                    self.move_plane(dy * 0.0005);
                    self.autosave();
                } else {
                    self.zoom(1.0 - dy * 0.002);
                }
                self.redraw();
            }
            WindowEvent::PinchGesture { delta, .. } => {
                self.zoom(1.0 - delta as f32);
                self.redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.applying.is_some() {
            self.check_apply();
            event_loop.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(250)));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

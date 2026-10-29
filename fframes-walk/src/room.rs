//! The room: a polished stone floor that mirrors, four walls and a ceiling, sized in metres,
//! with glTF models placed on its walls. A model marked as a window sits in an opening of
//! its wall, its colours glow as if lit from behind, and the sun shines in through it.

use fframes_three_d::{
    Background, Environment, Geometry, Light, Material, Model, NodeId, Reflector, Rgb, Scene,
    Vec3, vec3,
};

/// A wall of the room, seen from inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wall {
    Back,
    Front,
    Left,
    Right,
}

impl Wall {
    pub fn parse(s: &str) -> Option<Wall> {
        match s {
            "back" => Some(Wall::Back),
            "front" => Some(Wall::Front),
            "left" => Some(Wall::Left),
            "right" => Some(Wall::Right),
            _ => None,
        }
    }

    /// Rotation about y that turns a model facing +z to face into the room from this wall.
    fn yaw(self) -> f32 {
        match self {
            Wall::Back => 0.0,
            Wall::Front => std::f32::consts::PI,
            Wall::Left => std::f32::consts::FRAC_PI_2,
            Wall::Right => -std::f32::consts::FRAC_PI_2,
        }
    }
}

/// A model to place on a wall.
#[derive(Clone, Debug)]
pub struct Placement {
    pub path: String,
    pub wall: Wall,
    /// Height of the model in metres.
    pub height: f32,
    /// Height of its lower edge above the floor.
    pub sill: f32,
    /// Offset along the wall from its middle, metres (to the right seen from inside).
    pub offset: f32,
    /// Glow of its colours, as if lit from behind (a window); 0 = an ordinary object.
    pub glow: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct RoomSize {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
}

/// An opening in a wall: centre along the wall, bottom, width and height.
struct Opening {
    wall: Wall,
    along: f32,
    bottom: f32,
    width: f32,
    height: f32,
}

const WALL: f32 = 0.5;

/// The scene, and where to start walking.
pub struct Room {
    pub scene: Scene,
    pub size: RoomSize,
    pub start: Vec3,
    pub start_yaw: f32,
    /// Each window: its centre, the direction into the room, and its height.
    pub windows: Vec<(Vec3, Vec3, f32)>,
}

/// Builds the room with the models. Models that fail to load are reported and left out.
pub fn build(size: RoomSize, placements: &[Placement]) -> Room {
    let mut scene = Scene::new()
        .environment(Environment::studio().intensity(0.3))
        .background(Background::Color(Rgb::hex(0x0b0c10)));
    scene.reflector = Some(Reflector::floor());
    let (w, d, h) = (size.width, size.depth, size.height);

    let mut openings = Vec::new();
    let mut sun_from: Option<Vec3> = None;
    let mut windows = Vec::new();
    for p in placements {
        let bytes = match std::fs::read(&p.path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{}: {e}", p.path);
                continue;
            }
        };
        let model = match Model::from_gltf(&bytes) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{}: {e}", p.path);
                continue;
            }
        };
        let b = model.bounds();
        let (mw, mh) = (b.max.x - b.min.x, b.max.y - b.min.y);
        if mw <= 0.0 || mh <= 0.0 {
            eprintln!("{}: empty model", p.path);
            continue;
        }
        let s = p.height / mh;
        let width = mw * s;
        let inst = model.add_to(&mut scene, None);
        // the model's centre goes to the wall point; its front faces into the room
        let centre = (b.min + b.max) * 0.5;
        let yaw = p.wall.yaw();
        let rot = fframes_three_d::Quat::from_rotation_y(yaw);
        let half_wall = match p.wall {
            Wall::Back | Wall::Front => d / 2.0,
            Wall::Left | Wall::Right => w / 2.0,
        };
        // a window sits in the wall's thickness, an object just in front of the wall
        let inset = if p.glow > 0.0 { -WALL * 0.3 } else { 0.02 + (b.max.z - b.min.z) * s / 2.0 };
        let local = vec3(p.offset, p.sill + p.height / 2.0, -half_wall + inset);
        let at = rot * local;
        let root = &mut scene[inst.root];
        root.rotation = rot;
        root.scale = Vec3::splat(s);
        root.position = at - rot * (centre * s);
        if p.glow > 0.0 {
            for k in 0..inst.len {
                let id = inst.node(NodeId(k));
                if let Some(mesh) = scene[id].mesh_mut() {
                    mesh.cast_shadow = false;
                    mesh.receive_shadow = false;
                    let c = mesh.material.color;
                    // glass shows only the light coming through it: its colour times the
                    // daylight behind, not lit by the room; lead and dark paint stay dark
                    if c.0.x + c.0.y + c.0.z > 0.02 {
                        mesh.material = Material::unlit(Rgb(c.0 * p.glow)).double_sided();
                    } else {
                        mesh.material.roughness = 0.5;
                        mesh.material.metalness = 0.3;
                    }
                }
            }
            openings.push(Opening {
                wall: p.wall,
                along: p.offset,
                bottom: p.sill,
                width,
                height: p.height,
            });
            // the sun comes in through the first window, from outside and above
            sun_from.get_or_insert(rot * vec3(0.0, 1.2, -1.0));
            windows.push((at, rot * vec3(0.0, 0.0, 1.0), p.height));
        }
        eprintln!(
            "{}: {:.2} x {:.2} m on the {:?} wall",
            p.path, width, p.height, p.wall
        );
    }

    // floor and ceiling
    let floor = Material::standard(0x8a8378).roughness(0.35).reflection(0.35);
    scene.add(fframes_three_d::Node::mesh(Geometry::floor(w, d), floor).shadows(false, true));
    let ceiling = Material::standard(0x5a5550).roughness(0.9);
    scene.add(
        fframes_three_d::Node::mesh(Geometry::cuboid(w + 2.0 * WALL, WALL, d + 2.0 * WALL).translate(0.0, h + WALL / 2.0, 0.0), ceiling)
            .shadows(true, true),
    );

    // walls, with openings cut as rectangles
    let stone = Material::standard(0xb3aa9a).roughness(0.85);
    for wall in [Wall::Back, Wall::Front, Wall::Left, Wall::Right] {
        let (len, half) = match wall {
            Wall::Back | Wall::Front => (w + 2.0 * WALL, d / 2.0),
            Wall::Left | Wall::Right => (d, w / 2.0),
        };
        let holes: Vec<&Opening> = openings.iter().filter(|o| o.wall == wall).collect();
        let rot = fframes_three_d::Quat::from_rotation_y(wall.yaw());
        for (x0, x1, y0, y1) in wall_blocks(len, h, &holes) {
            let (bw, bh) = (x1 - x0, y1 - y0);
            if bw <= 0.001 || bh <= 0.001 {
                continue;
            }
            let c = rot * vec3((x0 + x1) / 2.0, (y0 + y1) / 2.0, -half - WALL / 2.0);
            let g = Geometry::cuboid(bw, bh, WALL)
                .rotate_y(wall.yaw())
                .translate(c.x, c.y, c.z);
            scene.add(fframes_three_d::Node::mesh(g, stone.clone()).shadows(true, true));
        }
    }

    // light: a dim interior, the sun through the window, a soft fill
    scene.add(fframes_three_d::Node::light(Light::hemisphere(
        Rgb::hex(0xbfc8d6),
        Rgb::hex(0x3a332b),
        0.15,
    )));
    let sun = sun_from.unwrap_or(vec3(0.5, 1.0, 0.6));
    scene.add(fframes_three_d::Node::light(Light::sun(Rgb::hex(0xfff2dc), 3.0, sun)));
    scene.add(fframes_three_d::Node::light(
        Light::point(Rgb::hex(0xffe2b8), 2.0, 0.0),
    ).with_position(vec3(0.0, h * 0.8, d * 0.25)));

    Room {
        scene,
        size,
        start: vec3(0.0, 1.7, d / 2.0 - 1.5),
        start_yaw: 0.0,
        windows,
    }
}

/// The rectangles of a wall `len` wide and `h` high around its openings (x from the middle,
/// y from the floor): for each opening the strips below and above it, and the pieces of wall
/// between the openings.
fn wall_blocks(len: f32, h: f32, holes: &[&Opening]) -> Vec<(f32, f32, f32, f32)> {
    let mut holes: Vec<&&Opening> = holes.iter().collect();
    holes.sort_by(|a, b| a.along.total_cmp(&b.along));
    let mut out = Vec::new();
    let mut x = -len / 2.0;
    for o in holes {
        let (l, r) = (o.along - o.width / 2.0, o.along + o.width / 2.0);
        out.push((x, l.max(x), 0.0, h));
        out.push((l, r, 0.0, o.bottom.max(0.0)));
        out.push((l, r, (o.bottom + o.height).min(h), h));
        x = r.max(x);
    }
    out.push((x, len / 2.0, 0.0, h));
    out
}

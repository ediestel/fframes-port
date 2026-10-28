//! The window as 3D glass tiles: the glTF export of `fframes-vector` loaded into
//! `fframes::three_d`. The camera orbits to show the tiles' thickness, then the objects
//! (top-level nodes of the export: `battle`, `banners`, `rose`, ...) slide out of the window
//! one after another and back, which only works because every object is its own node.
//!
//! Make the model first (run from this crate's directory):
//! `fframes-vector gltf vector/demo_objects.json vector/window_tiles.glb --depth 24 --tolerance 1`

use fframes::three_d::{
    Background, Camera, Environment, Light, Model, Renderer, Rgb, Scene, vec3,
};
use fframes::{Color, Duration, FFramesContext, Frame, Scene as FScene, Scenes, Svgr, Video};
use std::sync::OnceLock;

pub const WIDTH: usize = 1280;
pub const HEIGHT: usize = 720;
const FPS: usize = 30;
const SECONDS: f32 = 12.0;

/// Objects in the order they slide out.
const OBJECTS: [&str; 9] = [
    "rose", "banners", "battle", "spurs", "water", "land", "sky", "spandrels", "inscription",
];

struct World {
    scene: Scene,
    nodes: Vec<(usize, fframes::three_d::NodeId, fframes::three_d::Vec3)>,
    renderer: Renderer,
}

fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        let mut scene = Scene::new()
            .environment(Environment::studio())
            .background(Background::Transparent);
        let mut nodes = Vec::new();
        match std::fs::read("vector/window_tiles.glb").map(|b| Model::from_gltf(&b)) {
            Ok(Ok(model)) => {
                let inst = model.add_to(&mut scene, None);
                for (k, name) in OBJECTS.iter().enumerate() {
                    if let Some(id) = inst.find(&scene, name) {
                        nodes.push((k, id, scene[id].position));
                    }
                }
            }
            Ok(Err(e)) => eprintln!("vector/window_tiles.glb: {e}"),
            Err(e) => eprintln!("vector/window_tiles.glb: {e} (make it with fframes-vector gltf)"),
        }
        scene.add(fframes::three_d::Node::light(Light::sun(
            Rgb::WHITE,
            2.5,
            vec3(2.0, 3.0, 4.0),
        )));
        World {
            scene,
            nodes,
            renderer: Renderer::new(WIDTH as u32, HEIGHT as u32),
        }
    })
}

fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}

#[derive(Debug)]
pub struct TilesScene;

impl FScene for TilesScene {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(SECONDS)
    }
    fn name(&self) -> &'static str {
        "Tiles"
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}

#[derive(Debug)]
pub struct Tiles;

impl Video for Tiles {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(SECONDS)
    }
    fn audio(&self) -> fframes::AudioMap<'_> {
        fframes::AudioMap::none()
    }
    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![&TilesScene as &dyn FScene])
    }
    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let w = world();
        let mut s = w.scene.clone();
        // 4 to 10 s: each object slides 0.25 m forward and back, 0.35 s after the previous
        for &(k, id, rest) in &w.nodes {
            let start = 4.0 + k as f32 * 0.35;
            let out = ease((t - start) / 0.9) * (1.0 - ease((t - (start + 2.6)) / 0.9));
            s[id].position = rest + vec3(0.0, 0.0, 0.25 * out);
        }
        // the camera swings from the left to the right in the first 4 s, then rests at 32 deg
        let azimuth = -35.0 + 67.0 * ease(t / 4.0);
        let camera = Camera::perspective(35.0).orbit(vec3(0.0, 0.0, 0.1), 3.5, azimuth, 8.0);
        let image = w.renderer.render(&s, &camera);
        let href = image.href();
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT}>
                <rect x="0" y="0" width="1280" height="720" fill="#141320" />
                <image href={href} x="0" y="0" width="1280" height="720" />
            </svg>
        )
    }
}

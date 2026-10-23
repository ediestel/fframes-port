#![cfg(target_arch = "wasm32")]
use fframes_editor_controller::prelude::*;
use pixel_memory_example::rand::SeedableRng;
use pixel_memory_example::{rand, PixelMedia, PixelVideo, RandomPhotos};

impl_wasm_bridge_for!(PixelVideo<'static>, PixelMedia);

lazy_static! {
    static ref MEDIA: PixelMedia = PixelMedia::prepare().unwrap();
    static ref PHOTOS_LIST: Vec<String> = (2..=235).map(|i| format!("{i}.jpg")).collect();
    static ref VIDEOS_LIST: Vec<String> = (1..=22).map(|i| format!("{i}.mp4")).collect();
}

#[wasm_bindgen]
pub fn create_wasm_bridge() -> WasmBridge {
    console_error_panic_hook::set_once();

    // for more predictable results, you can use a fixed seed:
    // let mut range = rand::rngs::StdRng::seed_from_u64(28);
    let mut range = rand::thread_rng();

    let photos = RandomPhotos::new_from_static_list(&mut range, &PHOTOS_LIST, &VIDEOS_LIST);
    WasmBridge::new(
        PixelVideo::new_random_scenes(
            "naruto_grief.mp3",
            "Sometimes the smallest things take up the most room in your heart",
            &mut range,
            None::<&()>,
            photos,
        ),
        &MEDIA,
    )
}

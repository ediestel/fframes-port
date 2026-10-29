fn main() {
    // Fix 12: only probe FFmpeg libs when not targeting wasm32
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("wasm") {
        return;
    }

    for lib in [
        "libavcodec",
        "libavformat",
        "libavutil",
        "libswresample",
        "libswscale",
    ] {
        pkg_config::probe_library(lib)
            .unwrap_or_else(|e| panic!("pkg-config could not find {lib}: {e}"));
    }
}

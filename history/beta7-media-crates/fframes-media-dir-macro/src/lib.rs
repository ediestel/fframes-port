use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, LitStr, parse::Parse, parse::ParseStream};

/// Derive macro that generates a `StaticMediaProvider` impl.
/// The struct must have a `#[media_dir = "path/to/dir"]` attribute.
///
/// Generated impl calls `MediaDirectory::read_folder(path)?.process_media_source()?`
/// in `prepare()` and delegates all `MediaProvider` methods to the inner `DynamicMediaProvider`.
///
/// Example:
/// ```ignore
/// #[derive(MediaDir)]
/// #[media_dir = "assets"]
/// struct MyMedia;
/// ```
#[proc_macro_derive(MediaDir, attributes(media_dir))]
pub fn media_dir_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract #[media_dir = "..."] attribute
    let dir_path = input.attrs.iter().find_map(|attr| {
        if attr.path().is_ident("media_dir") {
            attr.parse_args::<LitStr>().ok()
        } else {
            None
        }
    });

    let dir_path = match dir_path {
        Some(p) => p,
        None => {
            return syn::Error::new_spanned(
                &input.ident,
                "#[derive(MediaDir)] requires a #[media_dir = \"path\"] attribute",
            )
            .to_compile_error()
            .into();
        }
    };

    let expanded: TokenStream2 = quote! {
        impl #name {
            fn __load_provider() -> fframes::error::Result<fframes::DynamicMediaProvider<'static>> {
                let dir = fframes::MediaDirectory::read_folder(#dir_path)
                    .map_err(|e| fframes::error::FFramesError::UserError(e.to_string()))?;
                dir.process_media_source()
                    .map_err(|e| fframes::error::FFramesError::UserError(e.to_string()))
            }
        }

        impl std::fmt::Debug for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, stringify!(#name))
            }
        }

        impl<'a> fframes::MediaProvider<'a> for #name {
            fn resolve_audio(&'a self, name: &str) -> Option<&'a fframes::AudioData<'a>> {
                self.0.resolve_audio(name)
            }
            fn resolve_image(&'a self, name: &str) -> Option<&'a fframes::media::ImageData<'a>> {
                self.0.resolve_image(name)
            }
            fn resolve_subtitles(&'a self, name: &str) -> Option<&'a fframes::media::Subtitles<'a>> {
                self.0.resolve_subtitles(name)
            }
            fn resolve_video(&'a self, name: &str) -> Option<&'a fframes::media::VideoMedia> {
                self.0.resolve_video(name)
            }
            fn populate_font_source(&'a self, font_source: &mut dyn fframes::FontSource) {
                self.0.populate_font_source(font_source)
            }
            fn populate_image_source(
                &'a self,
                image_data: &mut std::collections::HashMap<String, std::sync::Arc<fframes::usvgr::PreloadedImageData>>,
            ) {
                self.0.populate_image_source(image_data)
            }
        }

        impl fframes::StaticMediaProvider<'_> for #name {
            fn prepare() -> fframes::error::Result<Self> {
                // All media types (AudioData, ImageData, Subtitles) use PhantomData<&'a ()>
                // with fully owned internal data.  This makes DynamicMediaProvider covariant
                // in its lifetime parameter, so 'static is valid for runtime-loaded data.
                Ok(Self(Self::__load_provider()?))
            }
        }
    };

    expanded.into()
}

// ---------------------------------------------------------------------------
// static_media_dir! — compile-time directory scanner
// ---------------------------------------------------------------------------

/// Input for the `static_media_dir!` macro.
struct StaticMediaDirInput {
    struct_name: syn::Ident,
    dir_path: LitStr,
}

impl Parse for StaticMediaDirInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Accept either:
        //   static_media_dir!(MyMedia, "path/to/dir")
        let struct_name: syn::Ident = input.parse()?;
        let _comma: syn::Token![,] = input.parse()?;
        let dir_path: LitStr = input.parse()?;
        Ok(Self { struct_name, dir_path })
    }
}

/// Macro that scans a directory at compile time and generates a
/// `StaticMediaProvider` struct with embedded binary data via `include_bytes!`.
///
/// # Usage
/// ```ignore
/// fframes::static_media_dir!(MyMedia, "assets");
/// ```
///
/// This expands into:
/// - A `struct MyMedia(fframes::DynamicMediaProvider<'static>)` declaration.
/// - An implementation of `StaticMediaProvider` whose `prepare()` builds the
///   provider by calling `include_bytes!` for each file found in the directory
///   at compile time.
/// - Delegation impls for `MediaProvider` and `Debug`.
///
/// # Supported file types
/// The macro recognises files by extension and routes them to the appropriate
/// `MediaProvider` collection:
/// - `.png`, `.jpg`, `.jpeg`, `.webp`, `.gif` → images
/// - `.mp3`, `.wav`, `.ogg`, `.flac`, `.aac` → audio (loaded lazily at runtime
///   via `decode_raw_file`)
/// - `.mp4`, `.webm`, `.mkv`, `.mov`         → videos
/// - `.ttf`, `.otf`                          → fonts
///
/// Files with unrecognised extensions are silently ignored.
#[proc_macro]
pub fn static_media_dir(input: TokenStream) -> TokenStream {
    let StaticMediaDirInput { struct_name, dir_path } =
        parse_macro_input!(input as StaticMediaDirInput);

    let dir_path_str = dir_path.value();

    // Scan the directory at macro-expansion time (compile time).
    // `std::fs::read_dir` is available in proc-macro context.
    let entries = match std::fs::read_dir(&dir_path_str) {
        Ok(entries) => entries,
        Err(e) => {
            return syn::Error::new_spanned(
                &dir_path,
                format!("static_media_dir!: cannot read directory '{}': {e}", dir_path_str),
            )
            .to_compile_error()
            .into();
        }
    };

    let mut image_inserts = Vec::<TokenStream2>::new();
    let mut audio_inserts = Vec::<TokenStream2>::new();
    let mut font_inserts  = Vec::<TokenStream2>::new();
    let mut video_inserts = Vec::<TokenStream2>::new();

    for entry in entries.flatten() {
        let file_path = entry.path();
        if !file_path.is_file() {
            continue;
        }

        let file_name = match file_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Canonicalise to an absolute path string so `include_bytes!` can
        // find it regardless of where `cargo build` is invoked from.
        let abs_path = match std::fs::canonicalize(&file_path) {
            Ok(p) => p.to_string_lossy().into_owned(),
            Err(_) => continue,
        };

        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" => {
                image_inserts.push(quote! {
                    {
                        let bytes: &'static [u8] = include_bytes!(#abs_path);
                        match fframes::media::ImageData::new_from_bytes(#file_name, bytes) {
                            Ok(img) => { images.insert(#file_name.to_string(), img); }
                            Err(e) => {
                                // Non-fatal: log and continue so other media still loads.
                                eprintln!("static_media_dir: failed to load image '{}': {e}", #file_name);
                            }
                        }
                    }
                });
            }
            "mp3" | "wav" | "ogg" | "flac" | "aac" => {
                // Audio bytes are embedded via include_bytes! and decoded at runtime
                // by writing to a temp file (FFmpeg requires a seekable file path).
                // The temp file is removed after decoding.
                audio_inserts.push(quote! {
                    {
                        let bytes: &'static [u8] = include_bytes!(#abs_path);
                        let tmp_path = std::env::temp_dir()
                            .join(format!("fframes_audio_{}", #file_name));
                        if let Err(e) = std::fs::write(&tmp_path, bytes) {
                            eprintln!(
                                "static_media_dir: failed to write temp file for '{}': {e}",
                                #file_name
                            );
                        } else {
                            match fframes::media::PreloadedAudioData::decode_raw_file(
                                Some(44100),
                                &tmp_path,
                            ) {
                                Ok(data) => {
                                    audio.insert(
                                        #file_name.to_string(),
                                        fframes::AudioData::Preloaded(data),
                                    );
                                }
                                Err(e) => {
                                    eprintln!(
                                        "static_media_dir: failed to decode audio '{}': {e}",
                                        #file_name
                                    );
                                }
                            }
                            let _ = std::fs::remove_file(&tmp_path);
                        }
                    }
                });
            }
            "ttf" | "otf" => {
                font_inserts.push(quote! {
                    {
                        let data: &'static [u8] = include_bytes!(#abs_path);
                        fonts.push(fframes::RawFontData {
                            file_name: #file_name.to_string(),
                            data: std::sync::Arc::from(data),
                        });
                    }
                });
            }
            "mp4" | "webm" | "mkv" | "mov" => {
                // Videos cannot be embedded with include_bytes! in a portable
                // way because FFmpeg needs a seekable file path.  We embed the
                // path string and let VideoMedia::from_path handle it at runtime.
                video_inserts.push(quote! {
                    {
                        videos.insert(
                            #file_name.to_string(),
                            fframes::media::VideoMedia::from_path(#abs_path),
                        );
                    }
                });
            }
            _ => {} // Unrecognised extension – silently skip.
        }
    }

    let expanded = quote! {
        /// Auto-generated static media provider for the `#struct_name` directory.
        pub struct #struct_name(fframes::DynamicMediaProvider<'static>);

        impl #struct_name {
            /// Construct the provider by loading all embedded media.
            /// This is called by `StaticMediaProvider::prepare()`.
            pub fn load() -> fframes::error::Result<Self> {
                let mut images: std::collections::HashMap<String, fframes::media::ImageData<'static>> =
                    std::collections::HashMap::new();
                let mut audio: std::collections::HashMap<String, fframes::AudioData<'static>> =
                    std::collections::HashMap::new();
                let mut videos: std::collections::HashMap<String, fframes::media::VideoMedia> =
                    std::collections::HashMap::new();
                let mut fonts: Vec<fframes::RawFontData> = Vec::new();
                let subtitles: std::collections::HashMap<String, fframes::media::Subtitles<'static>> =
                    std::collections::HashMap::new();

                #(#image_inserts)*
                #(#audio_inserts)*
                #(#font_inserts)*
                #(#video_inserts)*

                Ok(Self(fframes::DynamicMediaProvider::new(audio, images, subtitles, videos, fonts)))
            }
        }

        impl std::fmt::Debug for #struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, stringify!(#struct_name))
            }
        }

        impl<'a> fframes::MediaProvider<'a> for #struct_name {
            fn resolve_audio(&'a self, name: &str) -> Option<&'a fframes::AudioData<'a>> {
                self.0.resolve_audio(name)
            }
            fn resolve_image(&'a self, name: &str) -> Option<&'a fframes::media::ImageData<'a>> {
                self.0.resolve_image(name)
            }
            fn resolve_subtitles(&'a self, name: &str) -> Option<&'a fframes::media::Subtitles<'a>> {
                self.0.resolve_subtitles(name)
            }
            fn resolve_video(&'a self, name: &str) -> Option<&'a fframes::media::VideoMedia> {
                self.0.resolve_video(name)
            }
            fn populate_font_source(&'a self, font_source: &mut dyn fframes::FontSource) {
                self.0.populate_font_source(font_source)
            }
            fn populate_image_source(
                &'a self,
                image_data: &mut std::collections::HashMap<String, std::sync::Arc<fframes::usvgr::PreloadedImageData>>,
            ) {
                self.0.populate_image_source(image_data)
            }
        }

        impl fframes::StaticMediaProvider<'_> for #struct_name {
            fn prepare() -> fframes::error::Result<Self> {
                Self::load()
            }
        }
    };

    expanded.into()
}

/// Attribute macro — passthrough; directory scanning is handled via #[derive(MediaDir)].
#[proc_macro_attribute]
pub fn static_media_dir_attr(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = attr;
    item
}

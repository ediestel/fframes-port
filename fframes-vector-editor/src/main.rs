//! `fframes-vector-editor pieces.json [zones.svg] [-o grouped.json] [--zones-out z.svg]
//! [--scale N] [--cut]`
//!
//! Draw, name and check object zones over a piece document. Grouping and the report of
//! doubtful pieces update after every change; `s` writes the zones (a zone SVG that
//! `fframes-vector group` reads) and the grouped document.
//!
//! Click a piece to select it (`Shift`+click adds), `Enter` to name the selection.
//! Keys: `z` draw a zone (click points, `Enter` to close, type the name, `Enter`),
//! `Backspace` removes the last point, `Esc` cancels; `Cmd+Z` / `u` undo and
//! `Cmd+Shift+Z` / `U` redo changes to the zones; `x` deletes the zone under the
//! pointer, `[` / `]` move it earlier / later (earlier wins); `g` picture, mix or groups;
//! `d` doubtful pieces on or off; `n` / `N` next / previous doubtful piece; `f` fit;
//! `c` cut pieces along the zones when saving; `s` save; `q` quit. Scroll or drag to pan,
//! pinch or Cmd + scroll to zoom.

mod editor;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// `dir/stem<suffix>` next to `path`.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let stem = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    path.with_file_name(format!("{stem}{suffix}"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(doc_path) = args.first().filter(|a| !a.starts_with('-')).map(PathBuf::from) else {
        eprintln!(
            "fframes-vector-editor pieces.json [zones.svg] [-o grouped.json] [--zones-out z.svg] [--scale N] [--cut]"
        );
        return ExitCode::FAILURE;
    };
    let zones_path = args
        .get(1)
        .filter(|a| !a.starts_with('-') && a.to_ascii_lowercase().ends_with(".svg"))
        .map(PathBuf::from);
    let config = editor::Config {
        out: opt(&args, "-o").map_or_else(|| beside(&doc_path, ".edited.json"), PathBuf::from),
        zones_out: opt(&args, "--zones-out").map_or_else(
            || match &zones_path {
                Some(z) => beside(z, ".edited.svg"),
                None => beside(&doc_path, ".zones.edited.svg"),
            },
            PathBuf::from,
        ),
        scale: opt(&args, "--scale").and_then(|s| s.parse().ok()),
        cut: args.iter().any(|a| a == "--cut"),
        zones: zones_path,
        doc: doc_path,
    };
    match editor::run(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fframes-vector-editor: {e}");
            ExitCode::FAILURE
        }
    }
}

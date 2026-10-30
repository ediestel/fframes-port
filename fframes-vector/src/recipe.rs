//! A figure's route as a recipe file (YAML): the source clip with its checksum, then the
//! steps, each a command, its output and the checks that output must pass.
//! `fframes-vector run recipe.yaml`, started in the repository root, runs the steps in order,
//! measures every result and stops at the first check that fails. A step whose output
//! already exists is refused, so a run never overwrites a figure. A step that names `code`
//! (a git commit) runs with `fframes-vector` built from that commit, so a recipe keeps the
//! exact version that made its figure. Commands, times and measurements go to a log beside
//! the recipe (`<recipe>.log`).
//!
//! In a step's `run`, `{source.clip}` is the source clip and `{fframes-vector}` the program
//! for that step, both quoted for the shell.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Free space a step needs to start, megabytes.
const MIN_FREE_MB: u64 = 2000;

#[derive(Debug, Deserialize)]
pub struct Recipe {
    pub figure: String,
    pub source: Source,
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Source {
    pub clip: String,
    pub md5: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub name: String,
    pub run: String,
    pub output: String,
    /// The commit whose `fframes-vector` runs this step; `None`: the running program.
    pub code: Option<String>,
    #[serde(default)]
    pub expect: BTreeMap<String, Check>,
}

/// A check on one measured value: a range, or an exact value.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Check {
    Range { min: Option<f64>, max: Option<f64> },
    Equal(serde_yaml::Value),
}

/// A measured value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Text(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Num(x) => write!(f, "{x}"),
            Value::Text(t) => write!(f, "{t}"),
        }
    }
}

impl Check {
    #[must_use]
    pub fn passes(&self, v: &Value) -> bool {
        match (self, v) {
            (Check::Range { min, max }, Value::Num(x)) => min.is_none_or(|m| *x >= m) && max.is_none_or(|m| *x <= m),
            (Check::Equal(e), Value::Num(x)) => e.as_f64().is_some_and(|y| (y - x).abs() < 1e-9),
            (Check::Equal(e), Value::Text(t)) => e.as_str().is_some_and(|s| s == t),
            _ => false,
        }
    }
}

impl std::fmt::Display for Check {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Check::Range { min, max } => match (min, max) {
                (Some(a), Some(b)) => write!(f, "{a}..{b}"),
                (Some(a), None) => write!(f, ">= {a}"),
                (None, Some(b)) => write!(f, "<= {b}"),
                (None, None) => write!(f, "any"),
            },
            Check::Equal(e) => write!(f, "= {}", serde_yaml::to_string(e).unwrap_or_default().trim()),
        }
    }
}

/// The number right after `key` in `text` (its first occurrence).
#[must_use]
pub fn number_after(text: &str, key: &str) -> Option<f64> {
    let at = text.find(key)? + key.len();
    let rest = text[at..].trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// The number right before `key` in `text` (its first occurrence).
#[must_use]
pub fn number_before(text: &str, key: &str) -> Option<f64> {
    let at = text.find(key)?;
    let head = text[..at].trim_end();
    let start = head.rfind(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).map_or(0, |i| i + 1);
    head[start..].parse().ok()
}

/// What a step's printed report says: the turn calibration of `depth`, the fit of
/// `splat-video`.
#[must_use]
pub fn report_values(text: &str) -> BTreeMap<String, Value> {
    let mut m = BTreeMap::new();
    let mut num = |k: &str, v: Option<f64>| {
        if let Some(v) = v {
            m.insert(k.to_string(), Value::Num(v));
        }
    };
    num("half_turn_frames", number_after(text, "half turn "));
    num("turn_degrees", number_before(text, "° in all"));
    num("axis_x", number_after(text, "axis x "));
    num("mirror_error_px", number_after(text, "mirror error "));
    num("direction_vote_chosen", number_before(text, " % of points in front"));
    num("direction_vote_other", number_before(text, " % the other way"));
    num("depth_known_percent", number_after(text, "depth known for "));
    num("voxels", number_before(text, " voxels ("));
    num("missed_percent", number_after(text, "missed "));
    num("excess_percent", number_after(text, "excess "));
    num("overlap", number_after(text, "overlap "));
    num("outline_distance_px", number_after(text, "outline distance "));
    if text.contains("turning anticlockwise") {
        m.insert("direction".into(), Value::Text("anticlockwise".into()));
    } else if text.contains("turning clockwise") {
        m.insert("direction".into(), Value::Text("clockwise".into()));
    }
    m
}

/// A command's output (stdout), or why it failed.
fn capture(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program).args(args).output().map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{program} failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn md5(path: &Path) -> Result<String, String> {
    capture("md5", &["-q", &path.to_string_lossy()])
}

/// The element count and SH degree in a `.ply` header.
fn ply_header(path: &Path) -> Result<(f64, Option<f64>), String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let end = data.windows(10).position(|w| w == b"end_header").ok_or("no ply header")?;
    let head = String::from_utf8_lossy(&data[..end]);
    let count = number_after(&head, "element vertex ").ok_or("no vertex count")?;
    Ok((count, number_after(&head, "SH degree:")))
}

/// What can be measured on a step's output itself.
fn output_values(path: &Path) -> Result<BTreeMap<String, Value>, String> {
    let mut m = BTreeMap::new();
    if path.is_dir() {
        let images = path.join("images");
        if images.is_dir() {
            let n = std::fs::read_dir(&images).map_err(|e| e.to_string())?.count();
            m.insert("images".into(), Value::Num(n as f64));
        }
        let points = path.join("points3D.ply");
        if points.is_file() {
            m.insert("start_points".into(), Value::Num(ply_header(&points)?.0));
        }
        return Ok(m);
    }
    m.insert("md5".into(), Value::Text(md5(path)?));
    match path.extension().and_then(|e| e.to_str()) {
        Some("mp4") => {
            let p = path.to_string_lossy();
            let s = capture(
                "ffprobe",
                &["-v", "error", "-count_frames", "-select_streams", "v:0", "-show_entries", "stream=width,height,nb_read_frames", "-of", "csv=p=0", &p],
            )?;
            let v: Vec<f64> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if let [w, h, n] = v[..] {
                m.insert("width".into(), Value::Num(w));
                m.insert("height".into(), Value::Num(h));
                m.insert("frames".into(), Value::Num(n));
            }
        }
        Some("ply") => {
            let (n, sh) = ply_header(path)?;
            m.insert("splats".into(), Value::Num(n));
            if let Some(sh) = sh {
                m.insert("sh_degree".into(), Value::Num(sh));
            }
        }
        _ => {}
    }
    Ok(m)
}

/// Quoted for `sh`.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn expand_home(s: &str) -> String {
    match (s.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => s.to_string(),
    }
}

fn free_mb() -> Result<u64, String> {
    let s = capture("df", &["-m", "."])?;
    s.lines().nth(1).and_then(|l| l.split_whitespace().nth(3)).and_then(|v| v.parse().ok()).ok_or_else(|| "df: no free space".to_string())
}

/// A shell command, its output shown as it comes and returned; fails on a non-zero exit.
fn run_shell(cmd: &str) -> Result<String, String> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(format!("{cmd} 2>&1"))
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("sh: {e}"))?;
    let mut text = String::new();
    if let Some(out) = child.stdout.take() {
        let mut reader = std::io::BufReader::new(out);
        let mut line = Vec::new();
        while reader.read_until(b'\n', &mut line).map_err(|e| e.to_string())? > 0 {
            let s = String::from_utf8_lossy(&line);
            eprint!("{s}");
            text.push_str(&s);
            line.clear();
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() { Ok(text) } else { Err(format!("exited with {status}")) }
}

/// The `fframes-vector` for a step: the running program, or one built from commit `code`
/// in its own worktree and build folder under `target/recipe` (built once, then reused).
fn program_for(code: Option<&str>) -> Result<PathBuf, String> {
    let Some(code) = code else {
        return std::env::current_exe().map_err(|e| e.to_string());
    };
    let tree = PathBuf::from(format!("target/recipe/{code}"));
    let build = PathBuf::from(format!("target/recipe/build-{code}"));
    let program = build.join("release/fframes-vector");
    if !program.exists() {
        if !tree.exists() {
            run_shell(&format!("git worktree add --detach {} {}", quote(&tree.to_string_lossy()), quote(code)))?;
        }
        run_shell(&format!(
            "cargo build --release -p fframes_vector --bin fframes-vector --manifest-path {} --target-dir {}",
            quote(&tree.join("Cargo.toml").to_string_lossy()),
            quote(&build.to_string_lossy())
        ))?;
    }
    std::fs::canonicalize(&program).map_err(|e| format!("{}: {e}", program.display()))
}

/// Runs a recipe from the current directory (the repository root). Returns the log path.
pub fn run(path: &Path) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let recipe: Recipe = serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let log_path = path.with_extension("log");
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("{}: {e}", log_path.display()))?;
    let mut note = |s: &str| {
        eprintln!("{s}");
        let _ = writeln!(log, "{s}");
    };
    note(&format!("== {} ({}), {}", recipe.figure, path.display(), capture("date", &[]).unwrap_or_default()));

    let clip = expand_home(&recipe.source.clip);
    if let Some(want) = &recipe.source.md5 {
        let got = md5(Path::new(&clip))?;
        if &got != want {
            note(&format!("source: md5 {got}, the recipe says {want}: stopped"));
            return Err("the source clip is not the recipe's".into());
        }
        note(&format!("source: md5 {got} as in the recipe"));
    }

    for step in &recipe.steps {
        let out = PathBuf::from(&step.output);
        if out.exists() {
            note(&format!("{}: {} exists already: stopped (a run never overwrites)", step.name, out.display()));
            return Err(format!("{} exists", out.display()));
        }
        let free = free_mb()?;
        if free < MIN_FREE_MB {
            note(&format!("{}: only {free} MB free: stopped", step.name));
            return Err("too little disk space".into());
        }
        let program = program_for(step.code.as_deref())?;
        let cmd = step
            .run
            .replace("{source.clip}", &quote(&clip))
            .replace("{fframes-vector}", &quote(&program.to_string_lossy()));
        note(&format!("{}: {cmd}", step.name));
        let start = std::time::Instant::now();
        let report = run_shell(&cmd).map_err(|e| {
            note(&format!("{}: {e}", step.name));
            format!("{}: {e}", step.name)
        })?;
        let mut values = report_values(&report);
        values.extend(output_values(&out)?);
        note(&format!("{}: done in {:.0} s", step.name, start.elapsed().as_secs_f32()));
        let mut failed = Vec::new();
        for (key, check) in &step.expect {
            match values.get(key) {
                Some(v) if check.passes(v) => note(&format!("  {key}: {v} (expected {check}) ok")),
                Some(v) => {
                    note(&format!("  {key}: {v} (expected {check}) FAILED"));
                    failed.push(key.clone());
                }
                None => {
                    note(&format!("  {key}: not measured (expected {check}) FAILED"));
                    failed.push(key.clone());
                }
            }
        }
        for (key, v) in &values {
            if !step.expect.contains_key(key) {
                note(&format!("  {key}: {v}"));
            }
        }
        if !failed.is_empty() {
            note(&format!("{}: stopped, checks failed: {}", step.name, failed.join(", ")));
            return Err(format!("{}: checks failed: {}", step.name, failed.join(", ")));
        }
    }
    note(&format!("== {}: all steps passed", recipe.figure));
    Ok(log_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_depth_and_splat_video_reports() {
        let depth = "screen colour [203, 203, 203], spread 8.7, 34146 figure pixels per frame, half turn 84.88 frames, axis x 372.5, turning radius 112 px\nx.glb: 30.4 MB, 192 frames, half turn 84.88 frames (2.121° per frame, 405° in all), axis x 372.5 (mirror error 1.75 px), turning anticlockwise (24.3 % of points in front of another frame's surface, 61.8 % the other way), depth known for 83 % of figure pixels, 578308 voxels (99.1 s)";
        let v = report_values(depth);
        assert_eq!(v["half_turn_frames"], Value::Num(84.88));
        assert_eq!(v["turn_degrees"], Value::Num(405.0));
        assert_eq!(v["mirror_error_px"], Value::Num(1.75));
        assert_eq!(v["direction"], Value::Text("anticlockwise".into()));
        assert_eq!(v["direction_vote_chosen"], Value::Num(24.3));
        assert_eq!(v["direction_vote_other"], Value::Num(61.8));
        assert_eq!(v["depth_known_percent"], Value::Num(83.0));
        assert_eq!(v["voxels"], Value::Num(578_308.0));
        let video = "out.mp4: 43113 splats, 192 frames at 2x, 24 fps; outline against the images (24 frames): missed 1.4 %, excess 5.3 %, overlap 0.937, outline distance 1.93 px (48.1 s)";
        let v = report_values(video);
        assert_eq!(v["overlap"], Value::Num(0.937));
        assert_eq!(v["outline_distance_px"], Value::Num(1.93));
        assert_eq!(v["missed_percent"], Value::Num(1.4));
        assert_eq!(v["excess_percent"], Value::Num(5.3));
    }

    #[test]
    fn checks_ranges_and_exact_values() {
        let r: BTreeMap<String, Check> = serde_yaml::from_str("a: {max: 3.5}\nb: {min: 0.9}\nc: 192\nd: anticlockwise").unwrap();
        assert!(r["a"].passes(&Value::Num(1.75)) && !r["a"].passes(&Value::Num(18.55)));
        assert!(r["b"].passes(&Value::Num(0.937)) && !r["b"].passes(&Value::Num(0.685)));
        assert!(r["c"].passes(&Value::Num(192.0)) && !r["c"].passes(&Value::Num(191.0)));
        assert!(r["d"].passes(&Value::Text("anticlockwise".into())) && !r["d"].passes(&Value::Text("clockwise".into())));
    }
}

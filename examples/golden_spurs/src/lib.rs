//! A Gothic stained-glass window of the Battle of the Golden Spurs (Kortrijk,
//! 11 July 1302), after the battle windows of the Church of Our Lady there.
//! Two lancets carry the scene: a French knight's horse founders in the ditch
//! while Flemish militia strike with goedendag and pike. The rose holds the
//! lion of Flanders, the two eyes beside it the golden spurs. One frame:
//! `golden-spurs frame 0`.

pub mod demo;
pub mod film;
pub mod tiles3d;
pub mod traced;

use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video};
use std::f32::consts::TAU;
use std::fmt::Write;
use std::sync::OnceLock;

pub const WIDTH: usize = 1200;
pub const HEIGHT: usize = 1800;

type Rgb = (u8, u8, u8);
type P = (f32, f32);

const LEAD: &str = "#15110d";
const PAINT: &str = "#2a180c";
const RUBY: Rgb = (150, 26, 30);
const AZURE: Rgb = (34, 66, 160);
const GOLD: Rgb = (226, 170, 40);
const GREEN: Rgb = (52, 122, 50);
const WATER: Rgb = (46, 128, 140);
const WHITE: Rgb = (232, 226, 206);
const DIM: Rgb = (186, 180, 162);
const STEEL: Rgb = (178, 186, 194);
const FLESH: Rgb = (240, 214, 186);
const WOOD: Rgb = (184, 128, 66);
const BROWN: Rgb = (110, 66, 30);
const SLATE: Rgb = (58, 80, 122);
const PURPLE: Rgb = (112, 58, 128);

const LANCETS: [(f32, f32); 2] = [(185.0, 575.0), (625.0, 1015.0)];
const L_SPRING: f32 = 880.0;
const L_BOTTOM: f32 = 1700.0;
const L_R: f32 = 300.0;
const BORDER: f32 = 22.0;
const ROSE: (f32, f32, f32) = (600.0, 395.0, 150.0);
const EYES: [P; 2] = [(300.0, 525.0), (900.0, 525.0)];

/// Lion rampant facing dexter in a 100 x 120 box, with its tail.
const LION: &str = "M30 10L45 8L52 18L50 28L60 38L70 55L72 70L80 78L78 92L88 108L96 112L84 114L72 100L64 96L60 108L66 116L54 116L52 104L50 92L44 80L40 66L34 60L20 64L10 60L18 56L30 54L34 46L24 44L12 38L20 34L32 38L34 30L26 30L18 26L22 20L26 14ZM74 66C92 60 98 40 88 26C96 36 92 54 78 62Z";
/// Fleur-de-lis in a 22-unit box centred on the origin.
const LILY: &str = "M0 -12C5 -6 5 -1 0 3C-5 -1 -5 -6 0 -12ZM-1.5 1.5C-8 -1 -13 -7 -8 -11C-10 -6 -6 -2 -1.5 -0.5ZM1.5 1.5C8 -1 13 -7 8 -11C10 -6 6 -2 1.5 -0.5ZM-6 1.5H6V4.5H-6ZM-2 4.5H2L3.5 10L0 7L-3.5 10Z";

fn hex((r, g, b): Rgb) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn pts(p: &[P]) -> String {
    p.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect::<Vec<_>>().join(" ")
}

/// Pointed arch from `x0` to `x1`, springing at `spring`, arcs of radius `r`.
fn arch(x0: f32, x1: f32, spring: f32, bottom: f32, r: f32) -> String {
    let mx = (x0 + x1) / 2.0;
    let half = (x1 - x0) / 2.0;
    let ay = spring - (r * r - (r - half).powi(2)).sqrt();
    format!("M{x0} {bottom}L{x0} {spring}A{r} {r} 0 0 1 {mx} {ay}A{r} {r} 0 0 1 {x1} {spring}L{x1} {bottom}Z")
}

fn interior(x0: f32, x1: f32) -> String {
    arch(x0 + BORDER, x1 - BORDER, L_SPRING, L_BOTTOM - BORDER, L_R - BORDER)
}

struct Doc {
    s: String,
    seed: u64,
}

impl Doc {
    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }

    /// The colour as one sheet of glass: each piece a little lighter or darker.
    fn vary(&mut self, (r, g, b): Rgb, amount: f32) -> String {
        let k = 1.0 + (self.rand() - 0.5) * 2.0 * amount;
        let c = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
        hex((c(r), c(g), c(b)))
    }

    fn raw(&mut self, t: &str) {
        self.s.push_str(t);
    }

    fn glass(&mut self, p: &[P], c: Rgb) {
        let fill = self.vary(c, 0.1);
        let _ = write!(
            self.s,
            r#"<polygon points="{}" fill="{fill}" stroke="{LEAD}" stroke-width="6" stroke-linejoin="round"/>"#,
            pts(p)
        );
    }

    fn path(&mut self, d: &str, fill: &str, w: f32) {
        let _ = write!(
            self.s,
            r#"<path d="{d}" fill="{fill}" stroke="{LEAD}" stroke-width="{w}" stroke-linejoin="round"/>"#
        );
    }

    fn circle(&mut self, x: f32, y: f32, r: f32, c: Rgb, w: f32) {
        let fill = self.vary(c, 0.08);
        let _ = write!(
            self.s,
            r#"<circle cx="{x:.1}" cy="{y:.1}" r="{r}" fill="{fill}" stroke="{LEAD}" stroke-width="{w}"/>"#
        );
    }

    /// A limb or shaft: a band of glass with its lead came around it.
    fn limb(&mut self, p: &[P], c: Rgb, w: f32) {
        let pts = pts(p);
        let fill = self.vary(c, 0.06);
        let _ = write!(
            self.s,
            r#"<polyline points="{pts}" fill="none" stroke="{LEAD}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/><polyline points="{pts}" fill="none" stroke="{fill}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            w + 11.0
        );
    }

    /// Grisaille: lines painted on the glass and fired.
    fn paint(&mut self, d: &str, w: f32) {
        let _ = write!(
            self.s,
            r#"<path d="{d}" fill="none" stroke="{PAINT}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round" opacity="0.85"/>"#
        );
    }

    fn lily(&mut self, x: f32, y: f32, size: f32) {
        let k = size / 22.0;
        let fill = self.vary(GOLD, 0.08);
        let _ = write!(
            self.s,
            r#"<g transform="translate({x:.1} {y:.1}) scale({k:.3})"><path d="{LILY}" fill="{fill}" stroke="{LEAD}" stroke-width="{:.2}" stroke-linejoin="round"/></g>"#,
            2.6 / k
        );
    }

    /// Lion of Flanders; `flip` turns it to face the other way.
    fn lion(&mut self, cx: f32, cy: f32, k: f32, flip: bool) {
        let (tx, sx) = if flip { (cx + 50.0 * k, -k) } else { (cx - 50.0 * k, k) };
        let _ = write!(
            self.s,
            r##"<g transform="translate({tx:.1} {:.1}) scale({sx:.3} {k:.3})"><path d="{LION}" fill="#17130f" stroke="{LEAD}" stroke-width="{:.2}" stroke-linejoin="round"/><polygon points="18,24 6,29 18,29" fill="#b8202a"/><polygon points="10,60 3,63 10,64" fill="#b8202a"/><polygon points="12,38 5,40 12,41" fill="#b8202a"/><circle cx="31" cy="17" r="2.2" fill="#e6b030"/></g>"##,
            cy - 60.0 * k,
            3.0 / k
        );
    }

    /// Prick spur with a star rowel, gold.
    fn spur(&mut self, x: f32, y: f32, s: f32, rot: f32) {
        let gold = self.vary(GOLD, 0.06);
        let rowel: Vec<P> = (0..16)
            .map(|i| {
                let a = i as f32 * TAU / 16.0;
                let r = if i % 2 == 0 { 12.0 } else { 4.5 };
                (31.0 + r * a.cos(), r * a.sin())
            })
            .collect();
        let band = "M-34 -13C-12 -15 0 -8 4 0C0 8 -12 15 -34 13";
        let _ = write!(
            self.s,
            r#"<g transform="translate({x:.1} {y:.1}) rotate({rot}) scale({s})"><path d="{band}M4 0L22 0" fill="none" stroke="{LEAD}" stroke-width="13" stroke-linecap="round"/><path d="{band}M4 0L22 0" fill="none" stroke="{gold}" stroke-width="6" stroke-linecap="round"/><polygon points="{}" fill="{gold}" stroke="{LEAD}" stroke-width="2.5"/><circle cx="-34" cy="-13" r="4.5" fill="{gold}" stroke="{LEAD}" stroke-width="2.5"/><circle cx="-34" cy="13" r="4.5" fill="{gold}" stroke="{LEAD}" stroke-width="2.5"/></g>"#,
            pts(&rowel)
        );
    }
}

fn window_svg() -> String {
    let mut d = Doc { s: String::with_capacity(512 * 1024), seed: 0x1302_0711_c0ff_ee11 };
    let _ = write!(
        d.s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">"#
    );
    defs(&mut d);
    wall(&mut d);
    head(&mut d);
    for (x0, x1) in LANCETS {
        lancet_border(&mut d, x0, x1);
    }
    d.raw(r#"<g clip-path="url(#lights)">"#);
    background(&mut d);
    city(&mut d);
    banner_france(&mut d);
    banner_flanders(&mut d);
    knight(&mut d);
    pikeman(&mut d);
    fighter(&mut d);
    for (x, y, s, r) in [(300.0, 1592.0, 0.7, -20.0), (450.0, 1590.0, 0.62, 160.0), (700.0, 1585.0, 0.7, 12.0), (880.0, 1588.0, 0.66, 190.0)] {
        d.spur(x, y, s, r);
    }
    inscription(&mut d);
    for y in [800.0, 1000.0, 1420.0] {
        let _ = write!(d.s, r#"<path d="M180 {y}H1020" stroke="{LEAD}" stroke-width="10"/>"#);
    }
    d.raw(r#"<rect width="1200" height="1800" fill="url(#glow)"/></g>"#);
    for (x0, x1) in LANCETS {
        let p = interior(x0, x1);
        d.path(&p, "none", 7.0);
    }
    d.raw(r#"<rect width="1200" height="1800" fill="url(#vignette)"/></svg>"#);
    d.s
}

fn defs(d: &mut Doc) {
    let (a, b) = (interior(LANCETS[0].0, LANCETS[0].1), interior(LANCETS[1].0, LANCETS[1].1));
    let _ = write!(
        d.s,
        r##"<defs><clipPath id="lights"><path d="{a}"/><path d="{b}"/></clipPath><clipPath id="caparison"><polygon points="{}"/></clipPath><radialGradient id="glow" cx="600" cy="1150" r="760" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#fff6d8" stop-opacity="0.18"/><stop offset="1" stop-color="#fff6d8" stop-opacity="0"/></radialGradient><radialGradient id="vignette" cx="600" cy="950" r="1150" gradientUnits="userSpaceOnUse"><stop offset="0.5" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.65"/></radialGradient></defs>"##,
        pts(&caparison())
    );
}

/// Ashlar wall in shadow, the splayed jamb, and the tracery stone of the opening.
fn wall(d: &mut Doc) {
    d.raw(r##"<rect width="1200" height="1800" fill="#24201c"/>"##);
    let mut y = 0.0;
    let mut row = 0;
    while y < 1800.0 {
        let mut x = if row % 2 == 0 { 0.0 } else { -75.0 };
        while x < 1200.0 {
            let w = 120.0 + d.rand() * 70.0;
            let c = d.vary((58, 52, 46), 0.14);
            let _ = write!(d.s, r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="66" rx="3" fill="{c}"/>"#, x + 2.0, y + 2.0, w - 4.0);
            x += w;
        }
        y += 70.0;
        row += 1;
    }
    let splay = arch(100.0, 1100.0, 780.0, 1780.0, 700.0);
    let jamb = arch(128.0, 1072.0, 780.0, 1752.0, 672.0);
    let opening = arch(150.0, 1050.0, 780.0, 1730.0, 650.0);
    let _ = write!(
        d.s,
        r##"<path d="{splay}" fill="#4a443c"/><path d="{jamb}" fill="#5c554b"/><path d="{opening}" fill="#3f3933" stroke="#6e665b" stroke-width="4"/><rect x="90" y="1752" width="1020" height="36" fill="#625a50"/>"##
    );
}

/// Rose with the lion of Flanders; the two eyes with a golden spur each.
fn head(d: &mut Doc) {
    let (cx, cy, r) = ROSE;
    let _ = write!(d.s, r##"<circle cx="{cx}" cy="{cy}" r="{}" fill="none" stroke="#6e665b" stroke-width="12"/>"##, r + 8.0);
    let inner = 106.0;
    for i in 0..16 {
        let (a0, a1) = (i as f32 * TAU / 16.0, (i + 1) as f32 * TAU / 16.0);
        let mut p: Vec<P> = (0..=6).map(|k| a0 + (a1 - a0) * k as f32 / 6.0).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect();
        p.extend((0..=6).rev().map(|k| a0 + (a1 - a0) * k as f32 / 6.0).map(|a| (cx + inner * a.cos(), cy + inner * a.sin())));
        d.glass(&p, if i % 2 == 0 { AZURE } else { RUBY });
        let am = (a0 + a1) / 2.0;
        let rm = (r + inner) / 2.0;
        d.circle(cx + rm * am.cos(), cy + rm * am.sin(), 6.0, WHITE, 2.5);
    }
    d.circle(cx, cy, inner, GOLD, 7.0);
    d.lion(cx, cy + 4.0, 1.38, false);
    for (x, y) in EYES {
        let _ = write!(d.s, r##"<circle cx="{x}" cy="{y}" r="50" fill="none" stroke="#6e665b" stroke-width="10"/>"##);
        d.circle(x, y, 43.0, AZURE, 7.0);
        d.spur(x - 4.0, y, 1.05, 0.0);
    }
}

/// Blue border with white pearls, the classic edge of a lancet.
fn lancet_border(d: &mut Doc, x0: f32, x1: f32) {
    let outer = arch(x0, x1, L_SPRING, L_BOTTOM, L_R);
    let _ = write!(d.s, r##"<path d="{outer}" fill="none" stroke="#6e665b" stroke-width="16"/>"##);
    let fill = d.vary(AZURE, 0.05);
    d.path(&outer, &fill, 7.0);
    let m = BORDER / 2.0;
    let mut pearls: Vec<P> = Vec::new();
    let mut y = L_SPRING;
    while y < L_BOTTOM - m {
        pearls.push((x0 + m, y));
        pearls.push((x1 - m, y));
        y += 34.0;
    }
    let mut x = x0 + m + 34.0;
    while x < x1 - m - 10.0 {
        pearls.push((x, L_BOTTOM - m));
        x += 34.0;
    }
    let rr = L_R - m;
    let (cl, cr) = (x0 + L_R, x1 - L_R);
    let t_apex = ((cl - (x0 + x1) / 2.0) / rr).acos();
    let mut t = 34.0 / rr;
    while t < t_apex - 0.04 {
        pearls.push((cl - rr * t.cos(), L_SPRING - rr * t.sin()));
        pearls.push((cr + rr * t.cos(), L_SPRING - rr * t.sin()));
        t += 34.0 / rr;
    }
    for (i, (px, py)) in pearls.into_iter().enumerate() {
        d.circle(px, py, 5.5, if i % 4 < 2 { WHITE } else { GOLD }, 2.5);
    }
}

/// Ruby ground of the scene, ditch water and grass, cut into irregular pieces.
fn background(d: &mut Doc) {
    let (cw, ch, ox, oy) = (52.0, 46.0, 178.0, 588.0);
    let (cols, rows) = (17usize, 24usize);
    let mut grid = vec![(0.0, 0.0); (cols + 1) * (rows + 1)];
    for j in 0..=rows {
        for i in 0..=cols {
            let jx = (d.rand() - 0.5) * 26.0;
            let jy = (d.rand() - 0.5) * 22.0;
            grid[j * (cols + 1) + i] = (ox + i as f32 * cw + jx, oy + j as f32 * ch + jy);
        }
    }
    for j in 0..rows {
        for i in 0..cols {
            let g = |a: usize, b: usize| grid[b * (cols + 1) + a];
            let quad = [g(i, j), g(i + 1, j), g(i + 1, j + 1), g(i, j + 1)];
            let (mx, my) = (ox + (i as f32 + 0.5) * cw, oy + (j as f32 + 0.5) * ch);
            let c = if my > 1572.0 {
                GREEN
            } else if my > 1502.0 {
                if mx < 640.0 { WATER } else { GREEN }
            } else {
                RUBY
            };
            d.glass(&quad, c);
        }
    }
    for y in [1518.0, 1540.0, 1560.0] {
        let mut w = format!("M196 {y}");
        let mut x = 196.0;
        while x < 640.0 {
            let _ = write!(w, "q9 -6 18 0t18 0");
            x += 36.0;
        }
        d.paint(&w, 3.0);
    }
    let mut grass = String::new();
    let mut x: f32 = 200.0;
    while x < 1000.0 {
        let y = if x < 640.0 { 1586.0 } else { 1520.0 + (x * 0.37).sin() * 6.0 };
        let _ = write!(grass, "M{x:.0} {:.0}l-4 -12M{:.0} {:.0}l3 -14", y + 14.0, x + 6.0, y + 14.0);
        x += 23.0;
    }
    d.paint(&grass, 2.5);
}

/// Kortrijk behind the Flemish line: the Broel towers over the Leie bridge and
/// the tower of the Church of Our Lady.
fn city(d: &mut Doc) {
    d.glass(&[(668.0, 800.0), (972.0, 800.0), (972.0, 852.0), (668.0, 852.0)], WHITE);
    let mut x = 672.0;
    while x < 966.0 {
        d.glass(&[(x, 788.0), (x + 12.0, 788.0), (x + 12.0, 800.0), (x, 800.0)], WHITE);
        x += 24.0;
    }
    d.paint("M690 852Q712 826 734 852M776 852Q798 826 820 852M862 852Q884 826 906 852", 3.5);
    for tx in [755.0, 885.0] {
        d.glass(&[(tx - 21.0, 728.0), (tx + 21.0, 728.0), (tx + 21.0, 852.0), (tx - 21.0, 852.0)], WHITE);
        d.glass(&[(tx - 27.0, 730.0), (tx, 672.0), (tx + 27.0, 730.0)], SLATE);
        d.paint(&format!("M{0} 748v14M{1} 748v14M{2} 790v18", tx - 7.0, tx + 7.0, tx), 3.5);
    }
    d.glass(&[(806.0, 716.0), (834.0, 716.0), (834.0, 800.0), (806.0, 800.0)], (236, 206, 140));
    d.glass(&[(800.0, 718.0), (820.0, 652.0), (840.0, 718.0)], SLATE);
    d.paint("M820 730v22M813 770h14", 3.0);
}

fn banner_france(d: &mut Doc) {
    d.limb(&[(232.0, 1330.0), (252.0, 752.0)], WOOD, 9.0);
    d.lily(252.0, 738.0, 26.0);
    d.glass(&[(254.0, 764.0), (404.0, 770.0), (390.0, 817.0), (410.0, 866.0), (256.0, 870.0)], AZURE);
    for (x, y) in [(282.0, 790.0), (330.0, 792.0), (376.0, 794.0), (304.0, 840.0), (352.0, 842.0)] {
        d.lily(x, y, 22.0);
    }
    d.paint("M300 770Q294 820 302 868M350 772Q356 820 348 866", 2.5);
}

fn banner_flanders(d: &mut Doc) {
    d.limb(&[(962.0, 1495.0), (958.0, 854.0)], WOOD, 9.0);
    d.glass(&[(952.0, 856.0), (958.0, 826.0), (964.0, 856.0)], STEEL);
    d.glass(&[(956.0, 862.0), (830.0, 858.0), (846.0, 908.0), (826.0, 960.0), (956.0, 962.0)], GOLD);
    d.lion(896.0, 910.0, 0.74, true);
}

/// Body and dagged hem of the French knight's caparison, in the knight's local frame.
fn caparison() -> Vec<P> {
    let mut p = vec![
        (-140.0, -2.0),
        (-60.0, -10.0),
        (0.0, -4.0),
        (60.0, -18.0),
        (115.0, -75.0),
        (165.0, -145.0),
        (176.0, -92.0),
        (140.0, -15.0),
        (126.0, 40.0),
        (118.0, 122.0),
    ];
    let mut x = 118.0;
    while x > -150.0 {
        p.push((x - 12.0, 138.0));
        p.push((x - 24.0, 122.0));
        x -= 24.0;
    }
    p.extend([(-156.0, 122.0), (-156.0, 30.0)]);
    p
}

/// The French knight, his horse pitching nose first into the ditch.
fn knight(d: &mut Doc) {
    d.raw(r#"<g transform="translate(330 1300) rotate(14) scale(0.86)">"#);
    d.limb(&[(-150.0, 10.0), (-184.0, 56.0), (-174.0, 110.0)], DIM, 16.0);
    d.limb(&[(-95.0, 118.0), (-80.0, 168.0), (-100.0, 208.0)], DIM, 17.0);
    d.limb(&[(70.0, 118.0), (98.0, 166.0), (124.0, 172.0)], DIM, 17.0);
    let cap = caparison();
    d.glass(&cap, AZURE);
    d.raw(r#"<g clip-path="url(#caparison)">"#);
    for (row, y) in (-150..130).step_by(42).enumerate() {
        let shift = if row % 2 == 0 { 0.0 } else { 20.0 };
        let mut x = -170.0 + shift;
        while x < 190.0 {
            d.lily(x, y as f32, 22.0);
            x += 40.0;
        }
    }
    d.raw("</g>");
    let _ = write!(d.s, r#"<polygon points="{}" fill="none" stroke="{LEAD}" stroke-width="6" stroke-linejoin="round"/>"#, pts(&cap));
    d.paint("M-110 20Q-118 70 -112 122M-60 10Q-66 70 -58 122M20 30Q14 80 22 122M84 20Q80 70 88 122", 3.0);
    d.limb(&[(-120.0, 118.0), (-132.0, 170.0), (-118.0, 212.0)], WHITE, 18.0);
    d.limb(&[(96.0, 118.0), (116.0, 164.0), (92.0, 196.0)], WHITE, 18.0);
    for (x, y) in [(-100.0, 210.0), (126.0, 174.0), (-118.0, 214.0), (90.0, 198.0)] {
        d.circle(x, y, 9.0, (60, 52, 44), 3.0);
    }
    d.glass(&[(222.0, -112.0), (248.0, -94.0), (252.0, -78.0), (236.0, -70.0), (218.0, -82.0)], WHITE);
    d.glass(&[(176.0, -150.0), (180.0, -180.0), (192.0, -156.0)], AZURE);
    d.glass(&[(160.0, -148.0), (184.0, -160.0), (230.0, -110.0), (226.0, -84.0), (198.0, -80.0), (170.0, -96.0)], AZURE);
    d.lily(206.0, -104.0, 15.0);
    d.circle(194.0, -130.0, 6.5, WHITE, 3.0);
    d.paint("M194 -130h0.5M240 -86q5 3 1 8", 4.0);
    d.paint("M238 -92L40 -64", 3.5);
    // rider
    d.limb(&[(2.0, -26.0), (40.0, 14.0), (30.0, 70.0)], STEEL, 24.0);
    d.glass(&[(18.0, 64.0), (54.0, 72.0), (52.0, 84.0), (16.0, 80.0)], STEEL);
    d.spur(14.0, 76.0, 0.55, 180.0);
    d.limb(&[(-72.0, -86.0), (150.0, -104.0)], WOOD, 11.0);
    d.glass(&[(146.0, -112.0), (160.0, -109.0), (154.0, -104.0), (166.0, -100.0), (148.0, -96.0)], WOOD);
    d.glass(&[(-30.0, -28.0), (-36.0, -112.0), (16.0, -122.0), (30.0, -30.0), (48.0, 4.0), (-38.0, 8.0)], GOLD);
    d.paint("M-20 -30Q-24 -10 -22 6M10 -28Q16 -8 22 4", 3.0);
    d.glass(&[(-14.0, -114.0), (46.0, -114.0), (46.0, -74.0), (16.0, -30.0), (-14.0, -74.0)], AZURE);
    for (x, y) in [(2.0, -88.0), (31.0, -88.0), (16.0, -62.0)] {
        d.lily(x, y, 17.0);
    }
    d.glass(&[(-12.0, -110.0), (44.0, -110.0), (44.0, -103.0), (-12.0, -103.0)], RUBY);
    for x in [-4.0, 12.0, 28.0] {
        d.glass(&[(x, -103.0), (x + 7.0, -103.0), (x + 7.0, -95.0), (x, -95.0)], RUBY);
    }
    d.glass(&[(-16.0, -172.0), (22.0, -172.0), (26.0, -128.0), (-18.0, -124.0)], STEEL);
    d.paint("M-14 -152L24 -152", 4.5);
    d.paint("M4 -170L4 -126M12 -142h0.5M18 -142h0.5M12 -136h0.5M18 -136h0.5", 3.0);
    d.lily(3.0, -186.0, 26.0);
    d.raw("</g>");
}

struct Militia {
    x: f32,
    y: f32,
    s: f32,
    tunic: Rgb,
    hose: [Rgb; 2],
}

/// A Flemish militiaman facing left, feet at the origin, about 290 units tall.
/// `weapon` is drawn between the body and the near arm.
fn militia(d: &mut Doc, m: &Militia, far: [P; 3], near: [P; 3], weapon: impl FnOnce(&mut Doc)) {
    let _ = write!(d.s, r#"<g transform="translate({} {}) scale({})">"#, m.x, m.y, m.s);
    let sleeve = (
        (m.tunic.0 as f32 * 0.8) as u8,
        (m.tunic.1 as f32 * 0.8) as u8,
        (m.tunic.2 as f32 * 0.8) as u8,
    );
    d.limb(&[(10.0, -128.0), (28.0, -66.0), (44.0, -10.0)], m.hose[1], 24.0);
    d.glass(&[(30.0, -16.0), (62.0, -12.0), (64.0, 0.0), (30.0, 0.0)], BROWN);
    d.limb(&[(-8.0, -128.0), (-40.0, -68.0), (-58.0, -12.0)], m.hose[0], 24.0);
    d.glass(&[(-88.0, -2.0), (-58.0, -18.0), (-46.0, -4.0), (-50.0, 2.0)], BROWN);
    d.limb(&far, sleeve, 20.0);
    d.circle(far[2].0, far[2].1, 10.0, FLESH, 4.0);
    d.glass(&[(-34.0, -218.0), (30.0, -220.0), (40.0, -170.0), (54.0, -106.0), (-52.0, -102.0), (-38.0, -170.0)], m.tunic);
    d.paint("M-20 -214L-30 -106M0 -216L0 -104M18 -216L30 -106", 2.5);
    d.glass(&[(-40.0, -162.0), (42.0, -164.0), (44.0, -150.0), (-42.0, -148.0)], BROWN);
    d.glass(&[(-14.0, -164.0), (-2.0, -164.0), (-2.0, -149.0), (-14.0, -149.0)], GOLD);
    d.glass(&[(-14.0, -222.0), (6.0, -222.0), (4.0, -212.0), (-12.0, -212.0)], FLESH);
    let face = d.vary(FLESH, 0.05);
    let _ = write!(d.s, r#"<ellipse cx="-6" cy="-242" rx="21" ry="25" fill="{face}" stroke="{LEAD}" stroke-width="5"/>"#);
    d.glass(&[(-25.0, -250.0), (-36.0, -236.0), (-24.0, -232.0)], FLESH);
    d.glass(&[(-27.0, -231.0), (-12.0, -215.0), (10.0, -221.0), (15.0, -240.0), (4.0, -231.0), (-12.0, -226.0)], (122, 74, 34));
    d.paint("M-19 -247q4 -3 8 0M-22 -255L-6 -257M-24 -228L-16 -229", 2.5);
    d.paint("M-15 -246h0.5", 4.0);
    let steel = d.vary(STEEL, 0.05);
    let _ = write!(
        d.s,
        r#"<path d="M-30 -258Q-28 -296 -4 -296Q20 -296 22 -258Z" fill="{steel}" stroke="{LEAD}" stroke-width="5"/><ellipse cx="-4" cy="-258" rx="40" ry="8" fill="{steel}" stroke="{LEAD}" stroke-width="5"/>"#
    );
    d.paint("M-4 -294L-4 -266", 2.5);
    weapon(d);
    d.limb(&near, m.tunic, 20.0);
    d.circle(near[2].0, near[2].1, 10.0, FLESH, 4.0);
    d.raw("</g>");
}

fn shaft_frame(p0: P, p1: P) -> (P, P) {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let len = (dx * dx + dy * dy).sqrt();
    let u = (dx / len, dy / len);
    (u, (-u.1, u.0))
}

/// Goedendag: a heavy club bound with iron and ending in a spike.
fn goedendag(d: &mut Doc, p0: P, p1: P) {
    let (u, n) = shaft_frame(p0, p1);
    let at = |t: f32, w: f32| (p1.0 + u.0 * t + n.0 * w, p1.1 + u.1 * t + n.1 * w);
    d.limb(&[p0, at(-72.0, 0.0)], WOOD, 13.0);
    d.glass(&[at(-72.0, 12.0), at(0.0, 14.0), at(0.0, -14.0), at(-72.0, -12.0)], (150, 98, 48));
    d.glass(&[at(-78.0, 15.0), at(-66.0, 15.0), at(-66.0, -15.0), at(-78.0, -15.0)], STEEL);
    d.glass(&[at(-8.0, 16.0), at(2.0, 16.0), at(2.0, -16.0), at(-8.0, -16.0)], STEEL);
    d.glass(&[at(2.0, 7.0), at(42.0, 0.0), at(2.0, -7.0)], STEEL);
}

fn pike(d: &mut Doc, p0: P, p1: P) {
    let (u, n) = shaft_frame(p0, p1);
    let at = |t: f32, w: f32| (p1.0 + u.0 * t + n.0 * w, p1.1 + u.1 * t + n.1 * w);
    d.limb(&[p0, p1], WOOD, 9.0);
    d.glass(&[at(-4.0, 6.0), at(18.0, 9.0), at(46.0, 0.0), at(18.0, -9.0), at(-4.0, -6.0)], STEEL);
}

/// Flemish militiaman bringing the goedendag down on the knight.
fn fighter(d: &mut Doc) {
    let m = Militia { x: 760.0, y: 1505.0, s: 1.0, tunic: GREEN, hose: [GOLD, WHITE] };
    militia(
        d,
        &m,
        [(18.0, -205.0), (34.0, -262.0), (-12.0, -323.0)],
        [(-22.0, -205.0), (-56.0, -262.0), (-29.0, -334.0)],
        |d| goedendag(d, (22.0, -302.0), (-150.0, -412.0)),
    );
}

/// Second militiaman behind, driving his pike into the horse.
fn pikeman(d: &mut Doc) {
    let m = Militia { x: 935.0, y: 1500.0, s: 0.86, tunic: PURPLE, hose: [WHITE, RUBY] };
    militia(
        d,
        &m,
        [(18.0, -205.0), (10.0, -160.0), (-19.0, -166.0)],
        [(-22.0, -205.0), (-50.0, -178.0), (-68.0, -174.0)],
        |d| pike(d, (70.0, -150.0), (-500.0, -250.0)),
    );
}

fn inscription(d: &mut Doc) {
    d.glass(&[(178.0, 1612.0), (1022.0, 1612.0), (1022.0, 1690.0), (178.0, 1690.0)], WHITE);
    let _ = write!(
        d.s,
        r#"<g font-family="Luminari" font-size="27" fill="{PAINT}" text-anchor="middle"><text x="380" y="1656">GULDENSPORENSLAG</text><text x="820" y="1656">KORTRIJK · 1302</text></g>"#
    );
}

static SVG: OnceLock<String> = OnceLock::new();

#[derive(Debug)]
pub struct Panel;

impl Scene for Panel {
    fn duration(&self) -> Duration<'_> {
        Duration::Frames(1)
    }
    fn name(&self) -> &'static str {
        "Window"
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}

#[derive(Debug)]
pub struct Window;

impl Video for Window {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }
    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }
    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![&Panel as &dyn Scene])
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::from_svg_string(SVG.get_or_init(window_svg).clone()).unwrap_or_else(|_| Svgr::empty())
    }
}

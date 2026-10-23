# Medical figures and cartoons with fframes

Notes on whether fframes can produce Thieme-Taschenatlas-style medical figures through LLM commands alone, and a
workflow for posable body outlines.

## 1. Can fframes render atlas-style figures?

Yes. Frames are plain SVG (`svgr!`), parsed by usvg and rendered by Skia on the GPU. The renderer supports
everything an atlas plate uses:

- flat colour fills and strokes, fine outlines
- linear and radial gradients for soft shading (`fframes-skia-renderer/src/render/convert.rs`)
- patterns for hatching and textures
- clip paths and masks for cutaways and sections
- SVG filters (`fframes-skia-renderer/src/render/filters.rs`)
- text, so labels, leader lines and legends work too

No SVG drawing library is needed: the agent writes `<path d="…">` and other SVG elements directly in Rust.

## 2. What an LLM can draw on its own

| Content | LLM-only quality |
| --- | --- |
| Labels, leader lines, numbered callouts, legends, plate layout | very good |
| Schematics: cell, synapse, nephron as tubes, signalling cascades, ECG loops, PK curves | good |
| Simple organs as icons (heart, kidney, liver, stomach) | fine as cartoons, not atlas-accurate |
| Real anatomy plates: muscle insertions, plexus, skull foramina, vessel courses | poor: proportions and topology drift |

The limit is not SVG. Writing freehand Bézier coordinates "blind" is hard for any LLM, and atlas style depends on
exact organic silhouettes.

### Ways to close the gap

1. **Look-and-fix loop.** `cargo run --release -- frame <scene>@50%` writes a PNG that the agent can open, compare
   with the description and correct. Helps a lot for cartoons and schematics, but won't reach anatomical accuracy.
2. **Rules instead of raw coordinates.** Shape functions such as `vessel(centerline, radius_profile)`,
   `muscle(origin, insertion, belly_width)` or mirrored organ outlines. An LLM gets parameters right far more
   reliably than path coordinates, and parameters animate well.
3. **Trace a reference.** Vectorise a sketch or photo with vtracer or potrace, then let the LLM simplify, recolour
   and label it.
4. **Licensed base art.** Servier Medical Art (CC-BY) and the Wikimedia anatomy SVGs give accurate outlines. The LLM
   handles styling, labels and animation.

Copyright: imitate the Thieme *style*, but don't copy or trace actual Thieme plates.

## 3. Why the fframes examples look so varied

The API itself is small:

- `svgr!`: SVG markup with Rust values in `{}`
- `Video` / `Scene`: one `render_frame(frame) -> Svgr` per frame, plus duration, overlap and audio
- media: images, fonts, audio, video, Lottie files
- optional GPU layers: SkSL/Shadertoy shaders and `fframes-three-d`

What makes it powerful is that each frame is a pure function of time written in ordinary Rust. Anything you can
compute (loops, maths, physics, randomness, data from files) you can draw.

The variety in the examples comes from two sources:

| Example | Source of richness |
| --- | --- |
| `fframes-intro`, `signal-lab`, `beta` | code: layout, easing, beat-synced timing |
| `shaders`, `shader-mode`, `neon-triangle` | GPU shader code |
| `arm`, `svg-in-3d` | the 3D crate |
| `pixel-memory`, `conference-splash-screen` | random or data-driven generation |
| `lottie` | 19 ready-made Lottie `.json` animations in `media/` |
| `made-of-motion` | an adaptation of a reference film, with a `hand-field.png` asset |
| `low-poly-art` | ~800 KB of pre-made polygon art in `src/low_poly_art/*.rs` |

Motion, layout, effects and data visuals come from code, which an LLM writes well. Detailed organic shapes come from
imported art. Medical figures follow the same split.

## 4. Posable body outlines: male and female, front and side

Goal: start from four base outlines, then let the LLM pose them ("lift the right arm").

### Why a single finished outline doesn't work

A front view drawn as one closed `<path>` is a long list of curve points. Lifting the arm means moving about 40 of
those points coherently and reshaping the shoulder and armpit. An LLM editing raw coordinates will distort it.

### Approach: rig once, then pose with angles

This is cutout (puppet) animation:

1. **Split** each view into body parts as separate paths: head, torso, upper arm, forearm, hand, thigh, lower leg,
   foot. Each part overlaps slightly into its neighbour.
2. **Pivot points** per joint: neck, shoulder, elbow, wrist, hip, knee, ankle.
3. **Hierarchy**: nested groups, torso › upper arm › forearm › hand, so rotating the shoulder carries the whole arm.
4. **Rotation around the pivot**: fframes' `Rotate` already has an `origin` (`fframes/src/transform.rs:112`).

A pose then becomes plain data:

```rust
Pose { r_shoulder: -150.0, r_elbow: -10.0, l_shoulder: 5.0, ..Pose::NEUTRAL }
```

Sketch of one arm chain (illustrative, not tested):

```rust
fn right_arm<'a>(p: &Pose, parts: &'a BodyParts) -> Svgr<'a> {
    fframes::svgr!(
        <g transform={rotate_about(p.r_shoulder, SHOULDER_R)}>
            {parts.upper_arm_r.clone()}
            <g transform={rotate_about(p.r_elbow, ELBOW_R)}>
                {parts.forearm_r.clone()}
                <g transform={rotate_about(p.r_wrist, WRIST_R)}>
                    {parts.hand_r.clone()}
                </g>
            </g>
        </g>
    )
}
```

"Lift the right arm" becomes changing one number, which an LLM does reliably. Animation comes free: blend between two
poses over time with easing. Organs, vessels or nerves drawn inside a part's group move with that part.

### Joint seams

The weak spot, mainly the shoulder at large angles.

- **Simple:** overlapping parts plus a round cap at each joint. Good enough for cartoon and Taschenatlas style.
- **Better:** a small skinning step in Rust (~100 lines). Every outline point gets weights for nearby bones and is
  rotated by their weighted transforms, which gives one smoothly deforming silhouette. fframes is plain Rust, so this
  can live in the project.

### Limits

- Front and side views are separate rigs; the LLM can't derive one from the other.
- Rotation works in the picture plane. A front view can raise an arm sideways but can't swing it toward the viewer
  convincingly; use the side view or a foreshortening trick.
- Base outlines: Wikimedia/CC body outlines or traced figure-drawing references. Splitting into parts and placing
  pivots is a one-time job per view.

### Division of labour

| Once, by hand or tool-assisted | Ongoing, by the LLM |
| --- | --- |
| 4 base outlines (male/female × front/side) | poses as angle data |
| splitting into parts, placing pivots | labels, leader lines, callouts |
| optional skinning weights | colours, highlights, organ overlays |
| | timing and animation between poses |

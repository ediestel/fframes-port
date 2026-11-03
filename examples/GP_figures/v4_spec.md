# Rig library v4: required fixes and acceptance checks

Source of truth for the look: the Gemini images (`thieme_atlas_style_reference_sheet.png`, `thieme_clinical_hand_matrix_template.png`, `thieme_clinical_foot_ankle_matrix_template.png`, `thieme_coronal_frontal_control_template.png`, `thieme_clinical_examination_couch_positions_template.png`, `thieme_pediatric_template.png`, `thieme_geriatric_sarcopenic_template.png`). v4 must be traced from these, not from v1–v3 shapes.

## Deliverables
1. One SVG file in the plain SVG namespace (no `ns0:` prefixes). Hands, feet and couches in `<defs>`, referenced by `<use href>`.
2. A PNG rendered from that exact SVG, plus a pose test sheet (check 15).
3. Header texts may only claim what the code does. v3 claimed "Perfected v1 Organic Anatomy", "True 3/4", "Left-facing profile & feet"; none of it held.

## Anatomy (all rigs A–E)
4. No capsule limbs. A limb path must not be two arcs joined by straight or near-straight sides. In v3: `thigh_far`, `shin_far`, `upper_arm_*`, `lower_arm_*` in rig A and every limb in B–E. Trace calf, knee, thigh, deltoid and forearm taper from the reference sheet silhouette, at least 20 cubic segments per limb.
5. Each segment owns its joint. The proximal end is a round cap centred on its `data-pivot`, radius = half the limb width there; the distal end tucks under the child's cap. No gap between torso and pelvis/thighs (v1: torso ends y 543, thighs start y 590, hip pivot y 555). No limb may start as a point (v1 upper arms start at (473,283) and (311,285)).
6. No spliced or reversed curves. In every cubic the first handle belongs to the start point and the second to the end point; no handle longer than its chord. v3 rig A `thigh_near` and `shin_near` break this (e.g. `C 384.83 601.00, 398.00 593.67, 388.00 590.00` from (380,555); control point at x 313 near (332,555)).
7. Neck: the torso top must cover the head base. v3 rig A head base is x 365–419 at y 244, torso top only x 365–377, which leaves a notch under the jaw.
8. One facing direction per rig, stated in the header, with head (nose, lips, chin), chest, feet and thumbs all agreeing. Rig A's head (copied from v1) shows no readable face. Rig D's head looks to +x (nose at x 416) while its foot points to −x.
9. Rigs D and E need both arms and both legs (near and far), like rig A. v3 has one of each.
10. Rig C must be a real three-quarter view: torso and pelvis foreshortened, far shoulder and hip offset, face turned. v3 rig C is a front figure with straight symmetric legs and front-view feet.

## Hands and feet
11. Hands as in the hand template: fingers, thumb and knuckles readable at body scale. Hand length 0.70–0.80 × forearm length, palm width ≥ forearm width at the wrist. v3 hands read as small mittens on the figures.
12. No fixed `rotate()` on a hand `<use>`. If one is needed, the hand axis must lie within 5° of the forearm axis (elbow pivot → wrist pivot). In v3 every hand rotation has the wrong sign: about 25° off the forearm.
13. Feet: keep the four v3 feet (they are good); toes must point in the rig's facing direction.

## Couches and style
14. Keep the v3 couches, their docking points and `data-surface-y`. Colours and line widths only from `style.txt` (charcoal outline, muted light blue, off-white, terracotta); `#6C8CAE` and `#9AB6CF` are not in it — replace or justify.

## Acceptance test
15. Pose test sheet rendered from the code: every rig with every joint at 0°, 45°, 90° and 120° in both directions. Pass = no gap, wedge, spike or detached part at any joint; hand and foot stay attached and aligned; the silhouette still matches the reference sheet.

# Elvar’s Teardown Method: A Detailed Workflow for Interactive Visual Explanations

The method turns a reference of a machine into an interactive explanation that runs in a browser, then uses the same scene to produce a guided video. The work proceeds through nine stages: install the toolkit, choose a mechanism and reference, build its model and geometry, add the explanatory console, establish the visual style, review the sequence, implement visitor control, render the video and sound, and publish the page.

The central rule is that the moving machine and its explanatory graphics share one model. The mechanism supplies the state; the 3D parts, diagrams, charts and labels display that state. A guided tour operates the presentation, and visitors can take over to explore it themselves.

The article describes six elements that work together:

| Element | Role in the explanation |
|---|---|
| Machine | The visible assembly, constructed from procedural 3D geometry. |
| Model | The relationships that determine part positions and changing quantities. |
| Panels | Charts, schematics and readouts that make those relationships visible. |
| Controls | Buttons, sliders and camera views through which the viewer explores the machine. |
| Diagnostic view | Traces, markers, transparency and overlays that expose otherwise hidden behavior. |
| Tour | A planned sequence that introduces the machine and demonstrates its controls. |

## 1. Install the toolkit

Elvar uses the `teardown` skill inside Claude Code. The repository combines instructions for the coding agent with Three.js examples, shared presentation utilities, a bundler, a frame-capture tool and a Python sound system. It builds on whaleyxbt’s claude-motion toolkit.

The article specifies Claude Code, Node.js 20 or newer, and Chromium through Playwright. Video export also uses FFmpeg; procedural sound uses Python 3.

Install the skill with the article’s command:

```bash
npx skills add elvarofficial/teardown -g -a claude-code
```

Install its dependencies and browser:

```bash
cd ~/.claude/skills/teardown && npm install && npx playwright install chromium
```

Open Claude Code and invoke `/teardown`.

Three example machines provide starting points: a cross-plane V8, Cessna 337 main landing gear and a Strandbeest linkage. From the installed skill folder, their interactive pages can be built with:

```bash
node sims/build.mjs v8-live
node sims/build.mjs cessna-live
node sims/build.mjs strandbeest-live
```

The build outputs go to `teardown-out/dist`. Start with the example closest to the intended explanation: the V8 for repeating cycles and internal activity, the landing gear for a constrained assembly driven by one input, or the Strandbeest for linked geometry and repeated parts.

## 2. Choose the machine, the question and the reference

Begin with a visible relationship the viewer should understand. In the landing-gear example, that relationship is the conversion of hydraulic motion into gear rotation and leg retraction. In the V8, it is the relationship between crank rotation, piston travel, valve timing and the four-stroke cycle. In the Strandbeest, it is the conversion of crank rotation into a walking foot path.

This focus determines which components deserve detail, which measurements belong on the console and which internal parts need to be exposed.

Collect a reference that shows the assembly clearly: a technical cutaway, an exploded drawing, a photograph or a video. Use it to establish proportions, component placement, connections and recognizable construction details. A video additionally supplies motion and sequence.

Prepare a component inventory before building geometry. Elvar’s landing-gear example specifies the main plate, hydraulic actuator, rack, pinion, output shaft, universal joints, side tubes, spring legs and wheels. This gives the agent an explicit assembly to construct and gives the creator a checklist for reviewing it.

Record the dimensions and operating quantities that drive the explanation. For the landing gear, those include rack travel, pinion tooth count and module. For an engine, they include bore, stroke, connecting-rod length and cycle timing. For a linkage, they include pivot locations and link lengths.

Keep each value associated with its source or modeling assumption. This makes later changes straightforward: a revised dimension updates a defined parameter rather than requiring scattered visual adjustments.

The output of this stage is a focused brief: the explanatory question, the reference, the component list, the driving input and the parameters needed to construct the mechanism.

## 3. Build the model, then the 3D assembly

### Establish the driving input

Choose the coordinate that describes the mechanism’s progression. The landing gear uses actuator stroke from 0 to 1. The engine uses crank angle. Each Strandbeest leg follows the crank angle through its connected links.

Define the endpoints and conventions explicitly. In the shipped landing-gear example, stroke 0 is down and locked; stroke 1 is retracted. A consistent definition makes controls, labels and tour events agree about what the machine is doing.

For machines with independently controlled motions, keep those inputs together in the shared model. The essential design choice is that all representations read the same state.

### Derive the connected motion

Calculate part positions from the driving input. In the landing gear, actuator stroke determines rack displacement, rack displacement determines pinion rotation, and that rotation carries through the shaft and side assemblies to the legs.

The article’s gear example uses 32 teeth and a 4 mm module. That gives a 128 mm pitch diameter. Half a revolution corresponds to half the pitch circumference, approximately 201.06 mm of rack travel. This hand calculation establishes the relationship that the visual explanation will show.

The V8 uses slider-crank geometry for piston travel and a 720° cycle for the four strokes. The valves and cycle indicators follow the appropriate cylinder phase. The pressure-volume display uses the same phase to position its marker.

The Strandbeest solver constructs joints through circle intersections: a joint connected to two known points must lie at the intersection of circles whose radii are the connecting link lengths. Repeating that operation establishes the leg pose for each crank angle.

### Make the model independently readable

Keep dimensions, motion relationships and derived quantities identifiable in the source. This makes the mechanism easier to inspect and lets geometry, schematics and readouts reuse the same relationships.

Changing speed should change how quickly the mechanism progresses. Changing a link length should alter every dependent joint position. A displayed displacement should come from the same calculation that positions the component.

### Construct the geometry

Build the visible parts from procedural geometry: cylinders for shafts, extruded profiles for plates and racks, lathed shapes for rotational components, and tubes or repeated instances for linked structures.

Organize parts according to their mechanical connections. A wheel belongs to its leg assembly; a piston follows its connecting mechanism; a valve moves along its own axis. This hierarchy makes it easier to pose the machine and to expose a subsystem cleanly.

Give important subsystems stable identities so that their buttons, labels, camera targets and cutaway modes refer to the same parts.

Add three complementary ways to inspect the assembly:

- **Isolation:** emphasize a selected subsystem and reduce competing detail.
- **Section or transparency:** expose internal activity while retaining the surrounding housing.
- **Explosion:** separate connected components so their arrangement becomes clear.

Maintain the underlying mechanism state while changing its presentation. The viewer should be able to return from an isolated or exploded view and recognize the same machine at the same point in its cycle.

## 4. Build the explanatory console

Elvar lays out the console on a 1600 × 900 design canvas. This provides a stable composition for the interactive page and the recorded film.

Place the title and the main live quantity prominently. For the engine, that quantity is RPM. Supporting graphics should answer specific questions about the movement on screen.

Use the console’s elements deliberately:

| Element | Question it answers |
|---|---|
| Main readout | What operating state is the machine in? |
| Cycle chart | Where are we in the repeating process? |
| 2D schematic | How do the connected parts relate? |
| Subsystem button | Which part should I inspect next? |
| Camera preset | Which viewpoint reveals this relationship? |
| Callout | What is this component called? |
| Trace or marker | Where has the part moved, or where will it go? |

For the V8, the valve-lift chart connects valve movement to crank phase, while the pressure-volume diagram connects the selected cylinder’s state to the cycle. A 2D schematic gives a simpler reading of relationships that can overlap in the 3D view.

Update the changing values from the frame’s shared state. Static specifications can come from the model’s parameter definitions; dynamic readouts follow the current pose or operating condition.

Connect selection across the interface. Choosing a cylinder should update the highlighted component, the tracked chart state and the relevant callout. Choosing a subsystem should reveal it and provide a useful camera angle.

Leader lines should terminate on the part they identify. Keep label placement readable as the camera and components move, especially during isolation and explosion.

## 5. Establish a visual style that explains the machine

Elvar’s default presentation uses a pale studio, soft shadows and one accent color. The lighting and materials give the machine a clear shape while keeping its working relationships easy to read.

Make the assembly occupy roughly 60–70% of its own viewport. This leaves room for context and annotations while keeping the important movement large enough to follow.

Use material differences to separate components. Dark housings, restrained metallic surfaces and transparent liners help distinguish structural parts from the active interior. The article recommends metal roughness of at least 0.45 to control bright reflections on large surfaces.

Give pale components enough contrast against the background. An outline, a darker ground or a change in lighting can preserve their edges. In the Strandbeest example, the repeated pale links need separation from the studio and from one another.

Assign the accent color a consistent meaning, such as the selected subsystem or the component currently doing work. Coordinate that emphasis with the corresponding panel or diagram.

Choose the camera according to the mechanism. Repeated planar links can overlap heavily in a direct view. A three-quarter angle along the assembly separates the structures and shows their depth. An engine needs a whole-machine view plus views that expose a cylinder, bank or internal valley.

Use close views to explain a connection, then return to a wider view so the viewer can place that detail within the assembly.

## 6. Design the tour and review its frames

Plan the sequence as an explanation with distinct beats. A useful progression is to introduce the whole machine, reveal the driving motion, isolate the relevant subsystem, expose its internal operation, change an operating input and return to the full assembly.

Each beat should produce a visible result. A button selects a part; a section reveals its interior; a slider changes speed; a new camera angle exposes a previously hidden connection.

Record the tour events and their times in a shared timeline. Camera movements, selections, control changes and sound accents can then refer to that sequence. Populate the event times before using the capture tool’s beat-review mode: the shipped example JSON timelines contain only duration, so those files need their actual beats added for that review command to cover the tour.

Render a contact sheet of the planned beats. Inspect every tile for:

- Components intersecting or separating from their mounts.
- Inconsistent rack, gear, shaft or linkage movement.
- Readouts that disagree with the visible state.
- Labels covering the component they describe.
- Important detail disappearing behind another part.
- Awkward framing, clipped text or insufficient contrast.

Inspect transitions in smaller time increments. Elvar specifies 0.2-second steps for reviewing jumps, sudden direction changes and abrupt camera or presentation changes. Use closer inspection wherever fast motion or tight clearances need it.

Correct the parameter, connection or transition responsible for the defect. Rebuild and render the affected sequence again, then review the complete tour. Inspect frames from the final encoded video as well, so the review covers the delivered result.

## 7. Make the page directly controllable

Use the same assembly and console for visitor interaction. Provide camera presets, subsystem buttons, sliders, orbit controls, zoom and a way to hide the panels.

The tour initially operates the interface through a visible cursor. It moves to controls and activates them, teaching visitors what they can do while showing the machine’s behavior.

A click, drag, scroll or key press hands control to the visitor. After 25 seconds without input, the tour resumes. Keep the handover coherent: an interaction should interrupt the tour, and the return should transition smoothly into the guided sequence.

Controls should expose meaningful relationships. The engine-speed control changes the pace of the cycle. The gear control exposes the connection between actuator travel and retraction. The Strandbeest wind control changes the crank speed and the corresponding walking motion.

Prepare the rendering pipeline before presenting the first visible scene. Warm the scene’s shader variants, prepare fonts and assets, and include presentation states such as transparency and cutaways that the tour will reveal.

Test the page on integrated graphics and at the intended display sizes. Elvar’s quality steps reduce ambient occlusion, pixel ratio, shadow-map size, sampling and panel redraw frequency. Profile expensive shaders as well: repeated work inside a shader can dominate rendering cost.

Evaluate performance while orbiting, switching views, exposing internals and running the mechanism. The page should remain responsive through the interactions that form the explanation.

## 8. Produce the video and sound

### Make the film seekable

The video pipeline asks the scene to render specific times. Elvar describes this through `render(t)`: requesting a time reconstructs the corresponding mechanism pose, camera, interface and presentation state.

Calculate the tour’s state from its timeline. Seed randomness used in scene construction. Integrate changing quantities in advance or reconstruct them from time, so seeking does not depend on which frame was rendered previously.

This contract lets the same scene produce an isolated still, a contact sheet, a transition inspection or a complete film.

### Render the frames

The capture tool opens the built film in headless Chromium, requests each frame time, captures it and sends the image to FFmpeg. A 26-second film at 60 fps contains 1,560 frames.

The article specifies 1920 × 1080 output, H.264 encoding, the `yuv420p` pixel format and `faststart`. The toolkit’s documented combination of a 1600 × 900 layout and a device-pixel ratio of 1.2 produces that output resolution.

From the installed skill directory, the documented V8 sequence includes:

```bash
node sims/build.mjs v8-film
node sims/capture.mjs v8-film --size 1600x900 --dpr 1.2 --stills beats --cols 4 --out teardown-out/v8-beats.png
node sims/capture.mjs v8-film --size 1600x900 --dpr 1.2 --out teardown-out/v8-silent.mp4
```

The beat-sheet command depends on the populated event timeline described in stage 6. Review the sheet before committing to the full export.

### Build the sound against the tour

Use the timeline to place button clicks, operating accents and transitions. In the V8 example, the musical bed uses Am9 harmony at 92 bpm, while a low thump follows each power stroke. As RPM rises, those mechanism-linked accents become more frequent.

Keep event timing shared between the film and score. The supplied V8 score duplicates several times from the JavaScript tour; consolidate those values when adapting the workflow so subsequent edits remain synchronized.

The sound should follow the explanatory hierarchy. A quiet click confirms a control action. An operating accent links a visible event to its timing. A restrained background bed supports the sequence without masking those cues.

Generate the score, master it to the article’s target of −14 LUFS with true peak below −1 dB, then combine it with the video. The toolkit’s sound instructions use a more conservative true-peak ceiling of −2 dBFS. Review the final combined file for event alignment, level and ending.

## 9. Publish the interactive explanation

Bundle the page for deployment. The standard build embeds the JavaScript, Three.js, CSS and fonts into the HTML. For machines with large model assets, place those assets alongside the page and include them in the deployed package.

Elvar uses an HTML size target below 1 MB to keep the entry page compact. Optimize the complete delivery as well: large external models still need to load before the relevant views can be shown.

Create a 1200 × 630 preview image from a frame that clearly communicates the mechanism. Add the page’s title, description, image URL and sharing metadata to the HTML head. The stock builder needs that publishing metadata added as part of deployment preparation.

Deploy the page and its assets to the intended host; Elvar uses Vercel. Check the deployed result from a fresh load, including fonts, model assets, tour startup, visitor takeover, controls, mobile layout and adaptive quality.

For an offline deliverable, test the packaged page and its accompanying assets without network access. For a public page, verify that the preview image and metadata are accessible at their published URLs.

## Extend the explanation with an exploratory control

The article’s final addition is a slider that lets the viewer change an operating condition and observe its consequences throughout the presentation.

In the Strandbeest, the wind control changes the rate of the mechanism. The walking motion, foot traces and associated panels follow that change. In the engine, a speed control changes how quickly the cycle progresses. In the landing gear, actuator stroke exposes the relationship between linear movement and rotation.

Choose the control according to the relationship being taught. Its value comes from giving the viewer a direct action whose consequences appear consistently in the machine, diagrams and readouts.

## Project organization

The repository separates the work into reusable roles:

| Source role | Responsibility |
|---|---|
| `kin.js`, `gear.js`, `jansen.js` | Dimensions, geometry relationships and mechanism state. |
| `engine.js`, `gear.js`, `beast.js` | Procedural construction and posing of the visible assembly. |
| `stage.js` | Camera, studio, lighting, shadows and post-processing. |
| `console.js` | Panels, diagrams, labels and readouts. |
| `film.js` | Tour events, camera sequence and time-based rendering. |
| `live.js` | Visitor controls, orbiting, tour handover and quality adaptation. |
| `score.py` | Sound generation and event placement. |
| `build.mjs` | Packaging the page and its assets. |
| `capture.mjs` | Still images, contact sheets and video capture. |

The examples group common implementation in a `-shared` directory and provide separate `-live` and `-film` entry points. This lets improvements to the machine and its explanatory graphics flow into both the interactive page and the video.

## Sources

This walkthrough develops the method in the user-supplied Elvar article and uses the public implementation at commit `e7e668cd44f48ba79a3c575e9f55c7e194d26eef` for architectural and command details.

- [Teardown repository](https://github.com/elvarofficial/teardown).
- [Workflow instructions](https://github.com/elvarofficial/teardown/blob/e7e668cd44f48ba79a3c575e9f55c7e194d26eef/SKILL.md).
- [Build implementation](https://github.com/elvarofficial/teardown/blob/e7e668cd44f48ba79a3c575e9f55c7e194d26eef/sims/build.mjs).
- [Capture implementation](https://github.com/elvarofficial/teardown/blob/e7e668cd44f48ba79a3c575e9f55c7e194d26eef/sims/capture.mjs).
- [V8 model](https://github.com/elvarofficial/teardown/blob/e7e668cd44f48ba79a3c575e9f55c7e194d26eef/sims/v8-shared/kin.js).
- [Landing-gear assembly](https://github.com/elvarofficial/teardown/blob/e7e668cd44f48ba79a3c575e9f55c7e194d26eef/sims/cessna-shared/gear.js).
- [Strandbeest linkage](https://github.com/elvarofficial/teardown/blob/e7e668cd44f48ba79a3c575e9f55c7e194d26eef/sims/strandbeest-shared/jansen.js).
- [Three.js renderer documentation](https://threejs.org/docs/pages/WebGLRenderer.html).

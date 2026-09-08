# Playable courtyard visual pass

Checkpoint: d923f71 (native input + measured idle pacing).

Keep Bevy, save format, grid footprints and input ownership. No dependencies or engine migration.
1. Lock startup selection/import/stress precedence with tests, then make the existing editable Blender pavilion the default experience; explicit --tower retains the original tower tutorial.
2. Reuse the six bounded Blender assets and improve their surface response without increasing draw submissions unnecessarily. Preserve deterministic exporter validation and source .blend.
3. Replace the oversized gray/flat backdrop with a bounded, layered courtyard composition, restrained water/vegetation and an architectural camera. Keep scenery outside building space and non-colliding.
4. Replace riverside blueprint voxel cells with component-sized guides while preserving exact placement footprints. Verify undo/remove/rebuild and never overwrite a loaded world.
5. Run Rust tests/clippy/fmt/build, asset validation; inspect native overview/night/close-up and record honest visual verdicts. Capture performance sanity checks on the same window configuration; no claims that 300% CPU is solved.

Acceptance: default entry visibly renders architectural assets; controls/UI remain readable; one editable pavilion can be dismantled/rebuilt, native frame pacing retained. A single vertical slice is not a full finished game.

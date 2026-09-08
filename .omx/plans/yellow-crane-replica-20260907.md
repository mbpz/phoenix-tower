# Yellow Crane Tower — MCP exterior reconstruction / native slice

1. Commit verified courtyard checkpoint (done: ef75eff, both ttbb).
2. Inspect connected Blender, preserve startup scene, create separate HHL_Reconstruction scene through MCP only.
3. Reference modern Wuhan Snake Hill tower: official Wuhan government describes 51.4 m height, 30 m bottom / 18 m top, five exterior storeys, yellow glazed eaves. Use official photographs to distinguish cross-shaped stacked eaves from generic square pagoda. Exact measured elevations / structural plans unavailable: mark estimated profiles and ornament, do not claim survey accuracy or interior replication.
4. Build deterministic original exterior geometry through Blender MCP. Seven independently named component roots: HHL_BASE, HHL_LEVEL_1..5, HHL_CROWN. Export metre-scale Y-up glTF Scene0 as assets/models/yellow_crane/yellow_crane.glb; native bounds nominal x/z +-18, y0..51.4. Source .blend and manifest retained. No external textures/asset downloads/dependencies.
5. Separate opt-in native --replica slice: actual imported scene, orbit/zoom, selected floor inspection, constrained bottom-up assembly rather than floating arbitrary pieces. Do not change courtyard/save v1. Tests lock assembly bounds and interaction math before edits. Remain opt-in until native visual acceptance.
6. Validate GLB structure/bounds/materials and source hierarchy, native fmt/test/clippy/build; inspect Blender and native screenshots each iteration and record verdict. Protect current unsaved courtyard process; do not reset it.

Not delivered by exterior slice: surveyed interior nine-floor layout, walkable stairs, exact decorative motifs, full park geography, photogrammetry/PBR surface capture, all-game acceptance or proven resolution of original 300% CPU report.

## User override: acquire dimensioned drawings first
2026-09-07: User requested original measured/design drawings before modeling. Pause estimated generator (moved to .omx/experiments/build_yellow_crane_unverified.py; NOT executed to create geometry). Located publicly reproduced design plates from 黄鹤楼设计纪事: https://hhl.cnkgraph.com/Picture/黄鹤楼设计纪事. Seventeen previews read and inspected; three priority originals downloading with bounded ranges. Treat intermediate elevations/roof profiles in earlier plan as unverified and superseded by drawing inspection. The separate Blender reference scene exists but has no building geometry yet. Do not describe these as certified as-built survey or DWG/CAD.

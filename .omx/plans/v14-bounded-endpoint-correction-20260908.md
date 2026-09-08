# v14 bounded correction / 2026-09-08

Scope adjustment after reading 2-2-13/14: full developed curve remains unresolved;
never use schematic dashed lines or multiply all plan lengths by sqrt(2).
Instead correct two separately transcribed, unconflicted symmetric endpoints:
L1 9.010/7.230 and L3 25.600/23.820 m. This does not complete curve calibration.

Cleanup plan: replace the inline L5-only rise branch with one small shared roof
construction entry point using explicit per-canopy tip constraints. Keep the pure
generic roof_mesh and all mesh sampling/formulas unchanged. No dependencies.

1. Add failing tests through the same geometry entry point the builder will use.
2. Constrain L1/L3, retain L5; L2/L4 remain byte-equivalent estimates because their
   annotations conflict. Preserve plan scales, posts, supports, crown and thickness.
3. Build new 14 via Blender MCP only after tests; preserve all old scenes/files.
4. Verify actual 24 new tip heights, 32 unchanged meshes, full L5 support footprints,
   topology and saved scene. Render matched front/oblique/underside views.
5. Inspect references + renders; keep M1 revise if shape/layers still insufficient.
6. Update handoff + hashes together, exclude private images and raw blend files.

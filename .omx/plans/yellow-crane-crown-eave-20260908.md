# Crown eave profile correction after d465a04

Scope: front trapezoid of each crown wing and its shared eave edge only. Preserve XY partition, ridge/hip boundaries, crown thickness, other 32 meshes and all previous scenes. No new dependencies, no game export.

1. Evidence: v07/source 2-2-7 registered crop has a straight central eave run in source versus a bowl-like generated line. Keep existing 39.3 m center/42.8 m tips and provisional 3.3 m half-ridge as a conservative shoulder control, NOT a surveyed eave width.
2. Regression first: flat central front eave, monotonic shoulder-to-tip rise, unchanged ridge/hip controls and all existing shell tests. Update the intended front surface regression only after the new constraint fails on v07.
3. Replace whole-width lift with shoulder-localized cubic lift on the same analytic trapezoid; one helper shared by edge and surface evaluation. No arbitrary triangulation smoothing.
4. Build v08 through Blender MCP, preserve prior scene signatures, render fixed front/oblique, compare source and v07. Record mesh counts and unchanged non-crown geometry.
5. Inspect lower-crown contacts separately. If visible clearance mismatch remains, report it, not a false pass from manifold checks. Do not combine unverified broader tier changes into this bounded correction.
6. Update checkpoint/evidence, distinguish local shape correction from overall M1 architectural acceptance.

# M1 v06 — crown seam repair

Baseline 1fc264f; previous v05 and source files remain untouched.

Diagnosis: five independent closed annular roof patches occupy overlapping plan area; a boolean union would retain internal sheets or distort the intended 2-2-8 side-wing footprint. Replace crown only with non-overlapping plan regions and shared seams, keeping named region IDs for the main sectors and four wings.

1. Reference gate: local 2-2-8 plan and v05 render; preserve raw manual controls and mark symmetry/height interpolation as estimated.
2. Regression first: top plan covers square minus finial aperture exactly once, matching seams share vertices, total shell closed/outward, no duplicated top projections, retained four wings/ridge heights, reject invalid resolution/thickness.
3. Pure geometry module: constrained plan patches, bounded triangulation/refinement, common edge profiles and one thickness shell. Reuse main builder's mesh/validation/material facilities; no new dependency or game export.
4. Build v06 via Blender MCP, compare v05/v06 old scene signatures, render front/oblique plus crown crop. Do not change lower roof tiers or band.
5. Inspect before next visual edit. Verify explicit crown seams/top coverage, not merely positive volume; independently review bounded code while rendering.
6. Document limitations: interpolated roof form is not measured; lower crown tier, ridge trim/contact, major fascia/supports and game/runtime remain unaccepted.

V06 visual gate: shared topology passes, but front-wing barycentric height blending creates central dimpling. Before v07, add a regression against a single analytic trapezoid surface; preserve ridge/eave/hip boundary profiles and all non-crown meshes.

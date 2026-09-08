# M1 v09 — bounded top-story support study

Commit v08 first: 245159c, ttbb <apples398@163.com>, not pushed.

## Scope / cleanup plan
Preserve all 33 v08 meshes and old scenes. Add one batched top-story support proxy mesh only; reuse existing box assembly, no dependencies. Do not build a continuous wall/fascia to hide roof gaps. No runtime/export changes.

Source 2-2-5 indicates stepped supports; not enough dimensions for measured dougong. Use L5 outer column XY from the existing design trace. Retain estimated post height 37.75m (not the 37.3m schematic level). Four stacked square masses (.48/.64/.84/1.04m wide) are explicitly estimates, not construction detail. Fit to the current canopy, not an asserted as-built roof.

## Tests before implementation
Clip actual downward-facing roof triangles to each support's largest square footprint. Minimum clipped Z bounds the flat cap across its entire footprint, unlike center-point sampling. Require complete projected coverage of the footprint; assumes a single-layer non-overlapping underside (true for supplied annular roof). Leave .02m fitting clearance. Reject invalid/nonfinite inputs, insufficient coverage or nonpositive height.
Test planar slope/interior vertex minimum, missing coverage, invalid data, contiguous tiers, finite geometry and 20 unique outer L5 columns. Run existing regression suite.

## Build / verification
Blender MCP fresh v09, preserve scene digests, compare all old mesh coordinates/faces to v08. Validate closed outward components and total cost. Render same front/oblique cameras plus support close-up. Persist visual verdict before next visual iteration. M1 remains revise until whole-building fidelity accepted; M2–M4 blocked on that acceptance.

## Preflight correction / interruption
Actual 09 build stopped at insufficient space before save. Pure fan triangulation
locates four corner conflicts: full-width cap ~37.765 vs post37.75. Do not shrink
the footprint or alter old roof to force a fit. Code now records blocked candidates,
with16accepted/4blocked in pure fixture. Two MCP diagnostic permission reviews timed
out (one retry). Revised code NOT yet run in Blender; latest complete08, next10.

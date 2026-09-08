# v14 actual post-footprint diagnostic (2026-09-08)

## Scope and preservation
- Attempt the requested previous v14 commit first using required identity. Automatic approval timed out twice; no Git write is claimed and no alternative permission bypass is allowed.
- Read existing v13/v14 geometry via Blender MCP. No roof/post edits, lowering columns, new exterior revision or save-over.
- Diagnose the prior 16 negative L3 bounding-square gaps with actual 16-sided post caps; distinguish genuine polygon overlap from conservative-square false positives.

## Implementation / cleanup plan
- Keep production square fitting helper and all existing geometry unchanged.
- Add a bounded read-only diagnostic: convex footprint/roof triangle intersection with extrema and coverage; inspect actual flat vertical prism assumptions before classifying any post.
- Tests first: sloped triangle minimum, square-only false positive, true overlap, partial/no coverage, malformed footprint, geometry unchanged.
- Reuse existing scene digest, actual Blender triangulation and report conventions. No dependencies.
- Independently verify reported overlaps against closed roof rays before calling them volume intersections, and compare v13/v14.
- Publish numerical evidence only, update handoff + manifest hashes. Private scene/source data stay ignored. M1 remains revise, game performance is out of scope.

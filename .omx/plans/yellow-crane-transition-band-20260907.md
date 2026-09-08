# M1 v05: ground transition band — bounded implementation plan

Baseline commit 3df5622, preserve all previous scenes and source files.

1. Source visual gate: inspect local 2-2-7 crop and v04 front. Overall remains revise, 76. Replace blank over-tall ground frieze only; defer crown clipping to independent next iteration.
2. Test-first: enforce band bounds below L2 slab, fourfold symmetry, positive finite member sizes, rectangular cell recess, no change to upper-floor enclosure heights, invalid outline rejection.
3. Reuse existing batched box geometry. Restrict ground enclosure height to existing 7.1 m post head. Add a stepped perimeter band derived from L2 outline, estimated sill 10.50–10.65, cells 10.65–11.45, header 11.45–12.15 m. Treat cells as recessed opaque proxies, not verified transparent windows.
4. Build new v05 with Blender MCP, preserve old scene signatures, render same front/oblique cameras; inspect cropped before/after and source.
5. Run regression, syntax, mesh checks; document remaining architecture/intersection/runtime gaps. Do not export or replace game asset.

# v13 bounded plan — L5 eave tip elevation

Preserve v12 and all older scenes/files. Plate 2-2-14 symmetric wing-eave drawing labels L5 tip 38.800 m and low eave 37.020 m. Remove only L5's inherited 3.5 m rise estimate (old tip 40.520 m); constrain tip to 38.800 m. Transfer to the existing twelve-tip mirrored proxy is provisional, not a verified correspondence for every corner or a developed roof curve.

1. Add failing regression for tip/eave/inner-ring elevations, closed topology, unchanged XY/faces/thickness; do not change generic roof default or other floors.
2. Apply L5-only endpoint constraint. Keep 1.0 plan scale, 37.750 post top, 1.04 support width, .020 clearance and >.040 minimum support height.
3. Blender MCP: snapshot old geometry digests/file hash, build v13, check actual tessellation and all support footprints, compare unchanged meshes, render same-camera front/oblique. No dependencies, extra subdivision, materials or game export.
4. Persist visual verdict (M1 still revise if curve/joins remain wrong), reviewed numeric evidence and handoff hashes. Test helpers/handoff and inspect diff independently.

Not covered: planar-to-developed metric curve mapping, asymmetric detail transfer, L4 conflicting labels, roof/post measured construction, full-model intersections, game acceptance.

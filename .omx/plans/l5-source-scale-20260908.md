# L5 source-plan scale correction (2026-09-08)

Scope: correct the extra 0.96 scaling of an outline already traced from L5 plate
2-2-4. This is not a clearance-driven roof lift or measured-construction claim.

1. Preserve v10 / diagnostic11 and all earlier scenes; new exterior12 only.
2. Record six manually checked SE eave landmarks on trace-plan-2-2-4.png,
   using the existing independent XY 18m axis calibration. Six-pixel tolerance
   covers the existing rounded outline, raster line width and manual selection;
   it is NOT a survey tolerance. Rotation to other quadrants remains estimated.
3. Add a regression that the actual L5 scale reproduces these landmarks within
   tolerance; observe failure at 0.96 before changing the scale to 1.0.
4. Keep other canopies, all heights, profile curvature, support width and clearance
   unchanged. Do not use support fit outcomes to select the scale.
5. Build through Blender MCP; compare old scene digests and v10 mesh scope,
   topology/volumes, full footprint clearances, then render front/oblique/closeup.
6. Record architectural revise unless the whole M1 visual gate is satisfied.
   Update handoff and selected evidence hashes; do not commit private plates or
   the multi-scene blend. New dependencies and game integration are out of scope.

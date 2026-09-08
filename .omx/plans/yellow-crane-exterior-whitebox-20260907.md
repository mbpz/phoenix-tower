# Exterior whitebox implementation plan

1. Preserve committed structural scenes/data; create a new exterior scene/file.
2. Trace roof plan topology separately from section elevations. Keep source pixel picks and inferred parameters explicit.
3. Add pure geometry helpers plus tests before Blender execution: closed roof patches, repeated-member batching, normal orientation, independent layer levels, no source textures.
4. Build distinct ground canopy / intermediate roof tiers / double upper roof and gable components. Whitebox-only estimates are tagged, not passed off as design or survey dimensions.
5. Render front and oblique cameras using Blender MCP, compare to source elevation, persist visual verdict before each corrective iteration.
6. Do not auto-export to game; M1 only passes if full silhouette reads correctly and remaining gaps are disclosed. M2 requires a frozen M1.

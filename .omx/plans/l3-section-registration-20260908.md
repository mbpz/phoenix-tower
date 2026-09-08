# L3 section registration boundary

Preserve exterior v14 and study 01. Inspect existing local original plates, record
source axis order and ground-plan section marker brackets separately from the
existing fixed-X diagnostic. The exact source cut path remains unresolved.

Implement a small read-only Blender probe reusing section_segments and scene
geometry digests: compare negative-Y mesh slices at two deliberately hypothetical
planes inside the marker bracket against the old positive-Y diagnostic. A missing
post slice is not clearance, and neither candidate plane is source registration.
Lock scope uncertainty with regression tests before implementing the probe.
Write new evidence only after actual Blender execution. Preserve historic inputs
and hashes, update handoff/manifest, run modeling and handoff tests. No production
geometry, dependency, portable export, or game-performance changes.

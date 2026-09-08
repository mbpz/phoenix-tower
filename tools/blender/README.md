# 黄鹤楼设计图结构研究（非游戏资产）

- `dimensioned_study.py`：纯 Python 网格函数 + Blender MCP 建模入口。
- `eave_study.py`：独立的非米制翼角像素描线场景。
- `test_*.py`：标准库单元测试，不导入 bpy，不启动 Blender。

测试（项目根目录）：

```sh
python3 -m unittest discover -s tools/blender -p 'test_*.py' -v
```

## 执行方式

实际 Blender 操作只能通过 Blender MCP 执行。`build_study(root)` 读取
`docs/refactoring/yellow-crane-plan-traces.json`，要求对应衍生预览已经合法取得、位于
`.omx/references/yellow-crane/`。原图和衍生预览未随脚本分发，不能直接在全新检出目录中运行。
仅查看 v08 白模不需要参考图库：使用[跨电脑接手包](../../docs/handoff/README.md)中的独立场景。

MCP Python 调用示例（将 `root` 设为当前项目绝对路径）：

```python
from pathlib import Path
root = Path('/path/to/phoenix-tower')
scope = {'__name__': 'hhl_local_study'}
script = root / 'tools/blender/dimensioned_study.py'
exec(compile(script.read_text(), str(script), 'exec'), scope)
scope['build_study'](root)
```

结构入口在创建场景前拒绝已有输出文件或同名场景，避免跨会话覆盖用户工作。
不要为绕过检查而删除研究文件：检查旧成果或在新的版本目录中重建。
翼角入口使用同样方式加载 `eave_study.py`，调用 `build_eave_study(root)`；只新增场景，
不自动保存，人工/MCP 检查后才保存当前正在编辑的研究文件。

## 打开与检查

成果 `.omx/references/yellow-crane/dimensioned-structure-study.blend` 默认展示整体。
通过 Blender 顶部 Scene 选择器切换 `HHL_Exploded_Study`、`HHL_Plan_Trace_QA`、
`HHL_Eave_Pixel_Study`。每个研究场景有独立相机；参考图需开启 Overlays。
整体/拆层可渲染，图像 Empty 参考仅显示于视口，不会出现在普通场景渲染中。

柱高/半径、板厚是展示代理；多数外围是轴线/栏杆线，不是最终板边。
翼角折线仅用于对照原图，禁止当作米制屋面扫描截面。
`export_to_game=false` 是语义标记，不会替代 Blender 导出器的人工筛选。
不要将任何研究场景或原图直接导出到游戏；完整限制与来源见研究登记文档。


## Exterior whitebox (M1 WIP)

`exterior_whitebox.py` reuses the study's slab/column helpers and creates a **new**
versioned scene via Blender MCP. Current local checkpoint is
`HHL_Exterior_Whitebox_08` / `.omx/references/yellow-crane/exterior-whitebox-08.blend`.
It has NOT passed architectural fidelity or game acceptance. See
`docs/refactoring/yellow-crane-exterior-whitebox-notes.md` for source/estimate boundaries.

Load this module with `tools/blender` on `sys.path`, then call `build(repo_root, '10')`
through Blender MCP for a new iteration. It refuses existing output files or scenes;
never delete previous versions to bypass the guard. If saving fails without creating
outputs, calling the same version retries saving its retained scene, not reconstruction.
If a partial output exists, preserve it and use a new version. Validation failures also
retain the new scene for inspection; correct code in a new version.

Roof panels are closed curved patches, but inter-object overlaps remain. Repeated
rails/windows/stairs are batched; no new dependency is needed for generation/tests.
`export_to_game=false` is metadata only, not an enforced exporter guard. No export is
performed by this module. Run all pure regression tests with:

```sh
python3 -m unittest discover -s tools/blender -p 'test_*.py' -q
```

Reports now verify non-manifold edges, winding consistency and signed volumes per
connected shell. These checks do **not** certify self-intersections, roof junctions,
walkable collision or real-time performance. Raw reference plates and local Blend/
overlay outputs must remain out of Git and game packages. The reviewed, scene-only
v08 snapshot under `docs/handoff/yellow-crane/` is a Git-only exception: it contains
no reference images and is still not an accepted game asset.

Checkpoint 05 limits the ground enclosure to the existing estimated post-head height
and adds a stepped rectangular-cell band below L2. Cells are **opaque recessed
proxies**, not verified windows/glazing. The band reuses batched boxes and the L2
floor outline; vertical bounds, subdivisions and member sections remain estimates.
Pure tests cover bounds, rotational/winding invariance, recesses and upper-story
height regression. Roof geometry is unchanged from checkpoint 04.

Checkpoint 07 replaces only the five crown sheets with `crown_whitebox.py`'s shared
shell. A fixed plan partition and common edge profiles remove overlap between its
main/wing top regions; sidewalls exist only on external boundaries. The front-wing
height is independent of triangulation diagonals. `crown_region` face IDs preserve
the five semantic regions. Reload `crown_whitebox` in the persistent Blender Python
session after editing it, before executing the builder again.

35 pure tests pass. The crown's topology is checked, **not architectural fidelity**:
mirrored plan controls, curves, main half-width and thickness remain estimates.
Contacts with the lower crown tier and ridge caps remain unverified. Checkpoint 07
has more triangles than 05; it is not a game-performance optimization. Keep all
intermediate scenes (including 06's visible interpolation artifact) for comparison.

Checkpoint 08 localizes front-wing corner lift outside provisional 3.3 m shoulders,
keeping a flat central run. The same helper evaluates the surface and eave boundary;
XY, face indices, region attributes and triangle count remain identical to 07.
This is an estimated silhouette constraint, not a newly measured eave dimension.
36 tests pass. The nominal shoulder is sampled by the existing mesh resolution;
no additional tessellation or modifier was added. Limited lower-crown clearance
sampling is diagnostic only, not an intersection/construction certificate.

Historical preflight (before checkpoint 10): `top_support_whitebox.py` clips a
single-layer underside against full square footprints, rather than center rays.
The builder uses actual Blender loop triangles; pure fixtures fan-triangulate quads.
Insufficient clearance is reported as blocked, never silently resized. Pure tests
identify 16 eligible outer columns and four corner conflicts; all sizes are estimates.
46 Python tests pass. The attempted 09 build stopped at its clearance guard before
save/render; subsequent MCP diagnostics timed out in permission review. Latest
complete checkpoint remains 08. Reload `top_support_whitebox` before retrying the
builder, use **10**, and preserve the incomplete 09 scene. No current support proxy
render, preservation proof, architectural acceptance or game-performance claim exists.

2026-09-08 handoff update: Blender MCP connectivity and the saved v08 scene-library
roundtrip were verified again. The permission timeout above is historical, not a
current blocker. No v09 build or visual acceptance was completed by packaging.

2026-09-08 modeling resume: the read-only MCP scene check succeeded, then checkpoint
10 was built, saved and rendered (front, oblique, underside closeup). Reload
`top_support_whitebox` and `crown_whitebox` **before** importing/reloading
`exterior_whitebox`; cached old modules otherwise fail at import. The old 16 scenes
and all 33 v08 baseline meshes were preserved. One new batched support mesh adds
64 boxes across 16 locations: 34 meshes / 106,776 triangles total. Actual loop-triangle
footprint checks give about 0.020 m minimum vertical clearance (float result
0.01999785 m); four corners remain explicitly blocked. Checks assume a single-layer,
non-overlapping underside projection, not an arbitrary layered roof.

M1 remains revise after visual inspection: stepped boxes are not measured dougong;
corner contact, eave curvature and structural correctness remain open. No runtime
export or performance claim. The portable Git scene remains v08; selected v10 JSON
evidence is in the handoff manifest, while raw v10 Blend/renders stay local-only.
Use a fresh version 11 or higher for the next iteration.


### Corner diagnostic study 11 (not exterior checkpoint 11)

`underside_probe` now exposes the clipped minimum point, source loop-triangle index
and projected coverage area. `underside_cap` delegates to the same calculation;
fit widths, heights, clearances and v10 geometry are unchanged. Three new witness
regressions bring the pure suite to 49 tests (all pass).

Via MCP, reload `top_support_whitebox` before importing/reloading
`corner_clearance_study`, then call `corner_clearance_study.build(root, '11')`.
It requires the original v10 scene and local saved v10 file; the portable v08 file
alone is insufficient. Existing scene/file names are rejected before modification.
A separate labeled analysis scene is saved and rendered; no building mesh is edited.
Use a fresh diagnostic number if outputs already exist; it is not a retry/overwrite API.

Actual Blender witness/barycentric checks, old 17-scene hashes, unchanged v10 file
and duplicate-run guard were verified. Four corners fall 44.8 mm short of the
**proxy fitting policy**, not architectural requirements. The section profile uses
41 one-millimeter square probes and non-uniform plot axes. The 37.300 m dark/mezzanine
level is not evidence for lowering corner posts. Study11 is not an exterior revision;
next exterior iteration is 12. Raw Blend/PNG remain local, selected JSON evidence
is listed in the handoff manifest. No scene-library roundtrip, independent review,
LSP, architectural acceptance or runtime performance verification is claimed here.


### Exterior12: retain L5's own plan scale

`FIFTH_LOWER_CANOPY_SCALE=1.0` removes the extra `.96` multiplier from an outline
already traced from L5 plate2-2-4. The six manual SE landmarks in
`docs/refactoring/yellow-crane-l5-eave-landmarks.json` reuse the original XY grid
calibration; the 6px tolerance is NOT survey accuracy. All six tests fail at .96.
Do not transfer this scale correction to other floors or change roof heights to
match a support-fit deficit. Blender12 has 20 proxy supports (0 blocked), 34 meshes,
106968 triangles. Only L5 roof/support meshes differ from10. M1 remains revise.
Private scenes/images remain ignored; reviewed numerical evidence is in handoff.


### Exterior14: independently constrained L1/L3 endpoints

`canopy_roof_mesh` is the shared production entry point for independently transcribed
L1 (9.01/7.23m), L3 (25.6/23.82m), and retained L5 (38.8/37.02m) tip/low elevations.
Do not spread these corrections to L2/L4: their source annotations conflict.
Intermediate Z remains estimated; XY/topology/thickness/ridge heights are unchanged.
Plate2-2-14 heights are true heights, but lengths require plan-based development;
the dashed non-setting-out curve is not calibration data. Mirrored twelve-tip
correspondence is still provisional. 53 helper tests pass; actual v14 saved/live
geometry and 120 endpoints were checked. Only L1/L3 meshes differ from13.

34 meshes /106968 triangles; all20 prior scenes retained. L5's20 support footprints
were rechecked using the same tested clipping helper. 68 lower-post bounding squares
were fully covered and checked;48 uncovered squares were NOT checked. No whole-scene
intersection or structural acceptance. Three rendered views keep M1 at82/revise.
Raw Blend/PNGs stay local; portable snapshot remains08. Use fresh version15 or higher.

Caution:16 of those68 covered squares already have negative L3 proxy gaps; some
worsen by up to3.004mm in14 (minimum -0.229167m). No newly negative square does NOT
mean clearance passes. Verify the actual post cross-section/roof contact next;
do not lower posts merely to hide this flag.


### Actual post-footprint preflight (read-only; runtime verification pending)

`post_contact_study.run_preflight(root)` returns a numerical report without writing
files or changing Blender. It uses generated vertical16-gon posts, convex clipping,
positive-area overlap rejection and strict prism side/cap topology checks. This is
separate from the production square fitter; no fitting policy changes.

The pure-source fixture has116 posts:68 full,8 partial,40 no-coverage;16 full L3
footprints remain negative. Quad fan triangulation and double-precision coordinates
are NOT saved Blender loop triangles. Do not label this actual mesh/solid readback.
MCP execution and escalated background startup approval timed out during this run.
Actual runtime contact validation remains the next step; the model is untouched.


## Post-contact stored-mesh investigation (v14, not acceptance)

`post_contact_readback.py` consumes extracted vertex/polygon arrays without changing
geometry. `analyze_stored_meshes(meshes)` deliberately checks both explicit quad
diagonals, **not** Blender's evaluated loop triangles. `solid_witness.py` independently
checks candidate interior points via signed solid angles; it rejects open/mis-wound
edges and boundary witnesses. It does not verify self-intersections or whole-scene
Boolean contact. `None` from witness search is inconclusive, never a clearance pass.

The local v14 diagnostic uses stored float32 arrays from the existing hashed `.blend`.
The one-file SDNA extraction script and raw arrays remain private/ignored; they are
not a supported file-format reader or a new portable asset. See the handoff report
for file hashes, extraction crosschecks, both-diagonal results and witness points.

For the next **actual Blender** audit, `capture_scene(scene, version)` can collect
world-space vertices, polygons and `loop_triangles` through MCP. This new capture
entry point has **not yet run** because runtime approval timed out. Use the returned
loop indices to build actual triangles for `polygon_probe` and `interior_witness`;
do not relabel `analyze_stored_meshes` output as a runtime result. The current helper
rejects parented, animated, constrained or modified objects rather than silently
inspecting a different shape. No model correction, save or export is performed.

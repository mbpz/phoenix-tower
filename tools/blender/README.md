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

MCP Python 调用示例（将 `root` 设为当前项目绝对路径）：

```python
from pathlib import Path
root = Path('/Users/jinguo.zeng/dmall/ai/phoenix-tower')
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

Load this module with `tools/blender` on `sys.path`, then call `build(repo_root, '09')`
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
overlay outputs must remain out of Git and game packages.

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

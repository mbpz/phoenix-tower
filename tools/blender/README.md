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

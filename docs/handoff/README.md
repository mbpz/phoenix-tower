# 跨电脑接手说明

更新：2026-09-08。本文件是持久交接入口，不是聊天记录备份，也不会自动恢复 OMX 执行模式。
历史笔记/计划如与本页状态冲突，以本页和重新取得的 Git、文件、测试证据为准。

## 先读什么

1. [AGENTS.md](../../AGENTS.md)：提交身份与协作约束。
2. [游戏 README](../../README.md)、[交付计划](../design/yellow-crane-delivery-plan.md)。
3. [白模研究说明](../refactoring/yellow-crane-exterior-whitebox-notes.md)、
   [来源登记](../refactoring/yellow-crane-reference-register.md)。
4. [.omx 历史计划](../../.omx/plans/README.md)：需要理解具体决策时再阅读。
5. [新电脑环境配置](environment.md)；只读白模不需要先恢复私人参考图库。

## 当前可以接手的状态

- 游戏仍是可编辑庭院原型；黄鹤楼完整复刻、美术与运行时验收未完成。
- 本次整理前代码基线是 `cdfc5de`，包含顶层支承净空检查。
  v08 建模提交为 `245159c`。历史文档中“本轮未提交”只描述当时，不代表当前状态。
- 本机最新已保存 Blender 检查点是 **v10：34 个网格，106,776 个三角面**。
  本次先成功只读检查，再通过 MCP 实建、保存和渲染；旧 16 个场景保留，v08 的 33 个网格不变。
- v10 新增 16 处简化柱头支承（64 个封闭方块合为一个网格），实际屋面三角化检查的最小垂直净空
  约 0.020 m（浮点结果 0.01999785 m）。四个转角仍因净空不足阻断；不是结构接触证明。
- 正面、斜视、檐下近景与剖面参考已检查；M1 仍为 **revise**，支承占位形态、转角关系与屋檐折痕
  未通过建筑外观验收。不是竣工实测模型或正式游戏资产；M2–M4 尚未开始。
- v09 未完成场景保留；未生成已验证的完整 v09。旧 MCP 权限审核超时已不再阻塞本次操作。
- **仓库可移植场景仍是 v08，不是 v10**。v10 原始多场景文件与渲染仅在本机忽略目录，
  本次只同步经过检查的 JSON 证据；clone 不会自动取得 v10 场景。

## 仓库携带的成果

[manifest.json](manifest.json) 列出需要随提交携带的文件、字节数和 SHA-256。

- [可移植 v08 白模](yellow-crane/exterior-whitebox-08-portable.blend)：约 0.93 MB，
  仅含生成的白模场景、网格、材质、相机和 World；不是原始多场景工作文件。
- [可移植性验证](yellow-crane/portability-verification.json)：通过 Blender MCP 从已保存的 v08
  隔离加载场景，提取依赖后重新加载导出文件，几何/变换/顶冠区域摘要一致。
  文件不包含图片、文本脚本、外部库、音频、视频、字体或驱动器；渲染路径改为相对路径。
  原 v08 文件、当前场景和原有 16 个场景的几何均保留。检查不覆盖每个 Blender 数据字段。
- [evidence](yellow-crane/evidence/)：v08 历史证据、v09 纯几何预检，以及本次 v10 的拓扑、
  旧场景保留、基线网格比较、实际支承净空和视觉结论。JSON 记录不能替代新电脑上的 Blender 复验。
- [source-inventory.json](yellow-crane/source-inventory.json)：来源图号、原链接、原图及研究预览哈希，
  不含原图文件，不自动下载。来源版本/精度/使用边界仍以来源登记为准。

这份白模的开放边界仅是“不携带参考图文件”，不应推断原出版物获得开放许可，
也不应将研究几何标作测绘级或已完成第三方权利审查的商业交付物。

## 新电脑如何继续

在需要的提交已推送后 clone，并进入仓库根目录。可先离线验证交接包：

```sh
python3 -B -m unittest discover -s tools -p 'test_handoff_assets.py' -v
python3 -B -m unittest discover -s tools/blender -p 'test_*.py' -q
```

Windows 可以将 `python3` 换成对应的 `python`。测试需要 Git，不需要联网、Blender 或新 Python 依赖。
第一条核对文件哈希、忽略规则和提取证据；不冒充一次新的 Blender 几何验收。

### 加载白模

已在 Blender 5.2.1 LTS 验证，其他版本需要重新验证兼容性。
建议在干净的 Blender 文件中使用 **File → Append → 可移植文件 → Scene →
HHL_Exterior_Whitebox_08_Portable**，再切换至该 Scene。这是已验证的 scene-library 恢复方式，
不依赖原电脑的工作区布局。也可通过 Blender MCP 执行：

```python
from pathlib import Path
import bpy
root = Path('/path/to/phoenix-tower')  # 改为当前机器仓库绝对路径
name = 'HHL_Exterior_Whitebox_08_Portable'
if name in bpy.data.scenes:
    raise RuntimeError('已存在该检查点，请使用现有场景，勿重复加载')
path = root / 'docs/handoff/yellow-crane/exterior-whitebox-08-portable.blend'
with bpy.data.libraries.load(str(path), link=False) as (available, requested):
    if name not in available.scenes:
        raise RuntimeError('检查点场景缺失')
    requested.scenes = [name]
bpy.context.window.scene = requested.scenes[0]
```

将后续工作另存到本地 `.omx/references/yellow-crane/` 的新版本文件，
不要直接覆盖受版本管理的检查点。

### 下一步建模

1. 检查当前场景、磁盘剩余空间和 MCP 连接；勿复制旧运行状态冒充恢复成功。
2. 阅读支承代码及测试。本机继续时使用新版本 **11**（如已存在则递增），保留08/09/10。
   先重载 `top_support_whitebox`、`crown_whitebox`，再导入/重载 `exterior_whitebox`，
   避免构建模块从缓存旧支承模块导入失败。新电脑只有可移植08，需明确重建与历史10的区别。
3. 核对四角屋面与柱头关系；不得通过缩小支承范围、抬屋顶或放松净空保护掩盖冲突。
4. 实际 Blender 运行后重新检查旧场景/33 个基线网格、拓扑、正面/斜视及支承近景。
   有新渲染才更新视觉结论；M1 通过之前不要开始游戏导出。

## 哪些不会随 clone 恢复

- 聊天全过程、内存中的 v09、当前窗口布局、临时权限状态和运行中的进程。
- `.omx/state/`、旧流水笔记、实验脚本、原始多场景 `.blend` 和备份。
- 原图、衍生预览、含原图的对比板：保留在旧机器的忽略目录；若需要图纸核验或重建参考板，
  必须在使用范围允许的前提下自行迁移或取得。结构研究脚本依赖清单中的预览文件，
  **不能仅靠 clone 重建私人参考板**。不要提交整个参考目录。
- 用户级 Codex 配置、技能/OMX 安装、Python、Blender 插件和凭据；参见环境说明。
- Git 仓库本地 `user.name` / `user.email` 配置；须在新 clone 中重新设置并核对。

## 维护规则

- `.omx` 默认忽略，仅白名单跟踪 `notepad.md` 和 `plans/*.md`。
  项目身份约束以 `AGENTS.md` 为准，不同步重复的机器专属 `project-memory.json`。
- 本次没有删除本地原图、旧成果或状态；旧笔记备份位于
  `.omx/state/notepad-before-context-portability-20260908.md`。
- 不将原始 `.blend` 直接拷入 Git。新检查点必须重复 scene-only 提取、依赖检查、复读比较，
  再更新清单；只保留有接手价值的检查点，避免把每次实验都变成永久二进制历史。
- 每次更新状态，区分“代码已实现 / 本次已验证 / 历史证据 / 尚未验证”；不要记“所有上下文已恢复”。
- 交接变更完成后提交并推送需要的分支；新电脑只能 clone 到远端已有的提交。

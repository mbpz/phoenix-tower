# 江岸样板后续风险收敛

基线：`37a5e7e`；2026-09-06。先提交已验证样板，再独立执行本轮。

## 先行计划与边界

1. 中文布局：检查锁定源码中的每帧文字/尺寸变化，先以回归测试复现重复失效，再只修应用侧可控写入。不删除中文，不过滤 stderr，不升级或 vendor 依赖。若上游构造器仍无配置入口，保留明确限制。
2. 性能证据：将现有 FPS 日志扩展为显式可选的连续采样；验证器对江岸样板支持采样窗口、最小样本及失败判定。先补工具测试，不把渲染压力实体、可编辑积木或 UI 初始化混为一谈。
3. 同条件比较：同一台机器、同一构建配置、相同预热/观测窗口，记录原生样板日志、中文诊断数量及 FPS。不声称跨平台、GPU 分析或全部人工通关。
4. 门禁：Rust/Python 回归、check、严格 Clippy、fmt、diff 检查、原生启动；涉及外观时保存截图并进行视觉复核。

并行边界：主执行方负责 UI 修复及最终记录；独立实现任务只负责 `src/ui/lunex/probes.rs`、`tools/verify_runtime.py`、`tools/test_verify_runtime.py` 中连续 FPS 证据与测试。保留原有存档与正常模式，禁止新增依赖。

## 已复现的应用侧根因（修复前）

- `collection_system` 每帧通过 `ResMut<Collection>` 调用 `Collection::update`，即使没有新解锁也会标记资源已变更。
- `b5_codex_status_sync` / `b6_achievement_sync` 仅检查资源的 Changed，随后无条件写入全部图鉴/成就的 `Text2d` 和 `UiColor`；隐藏面板仍在查询中。
- 锁定 `bevy_sprite 0.19.1` 的 `update_text2d_layout` 将 `text2d.is_changed()` 作为重新排版条件；锁定 `parley 0.9.0/src/analysis/mod.rs` 的 word/line segmenter 仍硬编码 `new_for_non_complex_scripts`。本轮不修改这些依赖。
- 三项失败测试已保存在 `/tmp/phoenix-collection-red.log`：重复 Collection 更新产生 4 个无意义文字变更（预期 0）；主题变更未更新行颜色；已选择图鉴解锁后详情仍显示未解锁。
- 修复策略：复用已有 `map_unchanged(...).set_if_neq(...)`，只在可见文字/颜色改变时使组件失效；详情补齐 Collection/BlockLibrary 依赖；颜色补齐主题依赖。不通过 `bypass_change_detection` 隐藏真实解锁，不改变收藏判定或自动进度保存。

## 实施与回归结果

- 提交检查点 `37a5e7e` 已完成，只提交项目代码/资产/文档，未提交 `.omx` 会话状态，也未推送远端。
- 生产 UI 只改三个同步系统：已有 equality-guarded 写入模式取代无条件文字/颜色写入；补齐详情和颜色的资源依赖。未改收藏判定、存档、模型或关卡。
- 新增 `--measure-fps`，普通/江岸模式显式注册约 1Hz 的 `Time<Real>` 采样器；压力模式仍只用原来的 2 秒采样器。保留所有原始 stderr，验证器检查生产时间戳和预热后的最少 3 个样本。该门槛只证明采样存在，不是 FPS 达标线。
- 3 项 Rust 回归先观察到断言失败，再修复：静止且隐藏的图鉴/成就不重写、解锁无需重新选中即更新详情、主题变化只更新颜色。中英混合详情字符串保持原文，但此测试不代替字体整形/断行视觉验证。
- 4 项 Python 新功能回归先失败，再通过：显式且不重复的采样开关、江岸缺样本失败、实时带时间戳样本通过、混入无时间戳样本仍失败。
- 完整 Rust **98 通过、0 失败、1 显式 ignored**；Python **34 通过**；locked offline build、all-target check、严格 Clippy、fmt、diff 检查通过。
- 独立只读复核：6 个源文件/测试文件，无可操作问题；未假称执行不可用的 LSP。复核不替代下述实际原生验证。

## 原生对照（2026-09-06，UTC+08:00）

本机 Apple M1 / Metal；`cargo build --locked --offline --profile test -j2`，不是 release。
修复前二进制保留 `37a5e7e` 的收藏 UI 实现，但加入与修复后完全相同的 FPS 探针；
构建后以独立文件保存，原 UI 修复逐字节恢复，再构建修复后版本。
两次都使用江岸模式、UI/样板就绪后预热 5 秒、观测 20 秒、不截图、不手动操作，顺序执行。
原生测量期间未并行运行 Cargo 编译/测试或其他本任务创建的游戏进程。

| 运行 | 完整进程时长 | 窗口内样本 | FPS 中位数 | 最低采样 FPS | 完整日志 CJK 警告 |
|---|---:|---:|---:|---:|---:|
| 修复前江岸 | 37.83s | 19 | 60.70 | 55.10 | 82,606 |
| 修复后江岸 | 32.90s | 20 | 59.95 | 52.50 | 2,026 |
| 修复后 50k 程序化渲染实体 | 34.75s | 10 | 31.35 | 30.20 | 1,730 |

50k 单独运行，预热 10 秒、观测 20 秒，精确确认生成 50,000 个渲染实体；
不是 50,000 个高细节江岸模型，也不是 50,000 个可编辑/物理积木。
三次均取得有效 UI/模式证据，完整窗口正常结束，未识别错误 0，子进程已回收。

解读边界：这是一次本机顺序配对的瞬时 FPS 采样，不是完整帧分布/平均吞吐/GPU 时间或统计显著性实验。
样板仍约 60 FPS，不能声称帧率显著提升。警告数为**完整运行**总数而非固定采样窗内数量，
启动就绪时间不同，不能直接以总数比例宣称每帧开销下降比例。测试及数量证据支持无意义重复排版已收敛，
但上游分词警告仍存在，50k 性能仍未作为达标验收。

原始日志/摘要目录（临时目录可能被清理；关键数据另存本仓库 JSON）：

- `/private/tmp/phoenix-risks-ab-before/20260905T184945Z-xi87zdyc/`
- `/private/tmp/phoenix-risks-ab-after/20260905T185159Z-akoj55h6/`
- `/private/tmp/phoenix-risks-50k/20260905T185322Z-i7rmli75/`

## 明确保留的风险

1. **上游中文分词**：Parley 构造器问题未解决；本轮未 vendor、升级或过滤日志。中文长段断行、日文及其他复杂脚本仍需单独布局验证。HUD 的动态数值变化也会合理地重新排版。
2. **规模性能**：50k 渲染样本仍约 31 FPS；没有 CPU/GPU 分段剖析，不能归因某个 shader 或保证长时、大编辑世界的尾延迟。后续先采集帧耗时/阴影/绘制成本，再决定优化，不能移除真实负载伪造指标。
3. **跨平台**：仅已有 Ubuntu/macOS/Windows CI 配置，本轮不 push，不宣称远端执行或 Windows/Linux GPU 验收。
4. **人工玩法**：自动化回归、UI 正尺寸日志和离屏 3D 截图不能替代整套鼠标/键盘、夜景、音频或全部蓝图人工通关。
5. **验证工具极端故障**：仍不是任意不可信二进制沙箱；无限无换行日志、二次中断等旧边界未在本轮扩大处理范围。没有新增存档按键或自动覆盖用户槽位。

## 最终补充证据

- 50k 可编辑数据回归也单独执行通过：50,000 个记录、200,000 占格，20 步撤销/重做与存档编解码往返；`/tmp/phoenix-risks-data-50k.log`。这是数据测试，不替代上表渲染测量。
- 修复后江岸截图与旧 `.ptw` 测试存档读入+截图分别通过：UI 正尺寸、样板 8 块就绪/对应文件读入日志、完整 PNG，未识别错误均为 0；各约 12.4 秒，CJK 分别 982 / 1,091 条。
- 实际查看新江岸截图并与已提交参考比较，未见主体、镜头、颜色或背景退化。此为离屏 3D 非回归，不含全 UI、中文断行或输入验收。视觉记录：`.omx/state/riverside-risk/ralph-progress.json`。
- 长期保留的机器可读证据：`docs/refactoring/riverside-risk-evidence.json`（含完整采样数组、运行摘要、二进制与截图 SHA256）。原始大日志继续保留在临时目录，不纳入 Git。
- 门禁原始输出：`/tmp/phoenix-risks-{build,tests,check,clippy,data-50k}.log`、`/tmp/phoenix-perf-green.log`。

### 本轮文件

- `src/ui/lunex/collection.rs`：三个 UI 同步系统的值比较与依赖补齐。
- `src/ui/lunex/tests.rs`：3 项行为/变更检测回归。
- `src/ui/lunex.rs`、`src/ui/lunex/probes.rs`：显式注册和连续 FPS 探针。
- `tools/verify_runtime.py`、`tools/test_verify_runtime.py`：可选帧率证据、4 项回归。
- `README.md`、`docs/design/riverside-slice.md`：命令和历史交付状态更新。
- 本记录与 `docs/refactoring/riverside-risk-evidence.json`：计划、验收、量化证据和剩余边界。

## 后续边界修复（基线 `22cc372`）

上述第 5 项中的无换行日志解析增长及 CLI 重复 SIGINT 回收问题已另行收敛，详见 `runtime-verifier-boundaries.md`。原始日志磁盘占用、派生子进程、强制杀死验证器等边界仍不属于沙箱保证；其余玩法、上游中文及跨平台风险保持不变。

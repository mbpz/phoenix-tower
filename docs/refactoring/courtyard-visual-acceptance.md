# 原生庭院视觉第一轮验收

日期：2026-09-07（macOS Retina，逻辑内容区 1280×720，scale=2）。
前置性能/输入修复提交 `d923f71`；本记录描述其后的庭院改动。

## 已实现、已验证

- 无参数进入八个真实积木组成的庭院；`--tower` 保留原教程，启动导入/压力测试不注入示例数据。
- 六个 GLB 共 2,438,112 字节；每件单 mesh/primitive，原始占格尺寸保持；内嵌 G=粗糙度、B=金属度图集区分石/漆/木/瓦/金。
- 静态环境复用 5 个 mesh、12 个材质，实体数回归约束 <240；没有新增环境碰撞体或逐帧生成背景。
- 庭院蓝图由 271 格可见体素变成 8 个整件预览；部分占用会隐藏整件，修改过的同名蓝图回退通用占格路径。
- 原生 CUA：Backspace 从 8 件到 7 件，M 显示屋顶预览，点击 `(690,278)` 恢复 8 件和 100%；不是通过测试探针直接插入积木。
- CUA 左拖 `(808,434)→(912,464)` 改变相机，积木仍为 8；滚轮缩放距离约 33.42→23.82，Home 恢复总览。
- T 日夜切换；F2 总览、近景、夜景均为 Bevy 实际离屏渲染，不是 Blender 静帧。PNG 不包含 UI；UI 可读性另以 CUA 原生窗口检查。
- F5 保存 8 件（槽位此前不存在），Backspace→7，F9 恢复 8 件及完整屋顶；日志分别有已保存/已加载 8 个积木。
- 中文子集覆盖代码和 RON 中全部 CJK + ASCII；移除知识卡片不受支持的 emoji 前缀，不引入 emoji 字体依赖。

截图：`../images/courtyard-day.png`、`../images/courtyard-closeup.png`、`../images/courtyard-night.png`。
它们记录最终 3D 参数；最后的知识卡片前缀/昼夜提示文字微调不影响 3D 渲染。

## 性能采样（不是 300% 问题结案）

本机优化 dev 构建，单实例 PID 75929，8 个积木、日景、前台静止、诊断日志开启。
2026-09-07 16:49:27–16:49:39：`top -l 7 -s 2 -pid 75929 -stats pid,cpu,mem,threads`。
丢弃首个初始化 0 样本，CPU 为 **54.2、66.2、66.4、66.4、65.0、63.7%**，均值 **63.65%**；top MEM 约 **493–494M**。
同窗口 13 个每秒瞬时 FPS 样本 **57.7–62.0**，均值 **60.14**。这不是 GPU 帧时分布、长时间平均吞吐或交互峰值测试。
原始日志：`/tmp/phoenix-courtyard-v3.log`、`/tmp/phoenix-courtyard-v3-top.txt`（临时诊断，不随仓库分发）。

**不能将这份新场景短测和用户原场景的约 300% 直接作同比，不能宣称下降百分比或根因已全部解决。**
后台截图看到约 10 FPS，符合此前空闲节流设计，但本轮没有新增后台 CPU 对照。

## 自动化证据

- Rust：161 passed、0 failed、1 ignored（ignored 为已有显式运行夹具生成测试）。
- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo build`。
- Python：43 项，含真实 GLB 二进制/PNG/占格约束、字体 cmap、验证器生命周期和入口参数。
- 知识卡片标题用先红后绿的定向测试确认，最终复用现有卡片回归并更新无 emoji 的预期；原字体缺字检查也先红后绿。
- 运行验证器普通 smoke 显式使用 `--tower`，`--riverside` 检查无参数入口，避免悄悄改变旧基准场景。

## 保留风险 / 未通过内容

1. 这是一份可编辑庭院视觉切片，不是完整游戏交付；没有在本轮完成从空地逐件搭建整亭、挑战、全修饰键组合及全部历史人工验收。
2. 法线/AO、木石微细节、生产级植被、水面反射/动态效果、黄鹤楼全套高质量资产仍缺。夜景以可辨认形体为先，不是写实照明。
3. 启动 `PHOENIX_LOAD` 会保留数据但使用程序化视觉；存档 v1 不持久化庭院模式。默认入口内 F9 恢复已验证。
4. ICU4X 缺少中文分词模型的诊断仍出现；字体覆盖修复并不解决这一独立上游问题。其他标签中的 emoji、任意导入文字不在字体覆盖保证内。
5. macOS 本机证据不代表 Windows/Linux、不同窗口宽高或 GPU 已验收。无长测、功耗或 GPU profile 数据。
6. 美术源仍是可复现脚本生成模块，GLB 缺失会警告并回退程序网格；不可把回退画面算作正常美术交付。

## 最终窗口状态

最终构建重新启动后，日志出现额外实时放置输入，当前 12 件；未再次撤销/读档覆盖该状态。
自由模式仍允许无支撑构件，多个屋顶可悬空放置（蓝图模式限制目标，但不是通用结构物理）；
这属于后续从零搭建/支撑规则验收，不能以本轮视觉提升掩盖。
最终提示“昼夜”已在窗口确认；卡片缺字前缀由回归验证删除，最终只读观察时卡片未显示。

## 主要变更文件

- 入口/相机：`src/main.rs`、`src/riverside.rs`、`src/camera/orbit_camera.rs`。
- 场景/光照/截图：`src/riverside_environment.rs`、`src/scene/mod.rs`、`src/screenshot.rs`。
- 蓝图/PBR：`src/building/placement.rs`、`src/building/tutorial.rs`。
- HUD/字体：`src/ui/hud.rs`、`src/ui/lunex/{layout,collection,tests}.rs`、`assets/fonts/`。
- 资产与验证：`tools/build_riverside_assets.py`、`tools/test_riverside_assets.py`、`tools/test_font_assets.py`、`tools/{verify_runtime,test_verify_runtime}.py`、`assets/models/riverside/`。
- 文档与图像：`README.md`、`docs/refactoring/visual-direction.md`、本文及 `docs/images/courtyard-*.png`。

简化：整件蓝图替代体素堆；复用材质/网格而不是逐装饰独立资产；保留单 primitive 与已有存档路径；
截图相机只在触发时复制变换/雾参数；删除不必要的 emoji 前缀和冗长 HUD 文本。

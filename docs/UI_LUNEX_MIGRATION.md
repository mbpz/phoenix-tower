# UI 迁移计划：bevy_egui → Bevy-Lunex（参考 Bevypunk）

> 状态：✅ 主体完成（2026-09，A/B/C/D 全部落地，待真人 playtest 收尾）· 目标：用 Bevy-Lunex（retained/ECS 布局引擎）重构游戏 UI 交互，
> 参考 Bevypunk（IDEDARY/Bevypunk，Cyberpunk UI 复刻，bevy ^0.19.1 + bevy_lunex）。
> 兼容性：bevy_lunex 0.7.0 依赖 bevy ^0.19（与本项目 0.19.1 一致 ✓）。

## 背景与动机

当前 UI 为 bevy_egui（立即模式、独立覆盖层、工具风、不与 3D 场景融合）。
Lunex 为保留模式、ECS 组件驱动、走 Bevy 自身渲染管线——上限更高（世界空间 UI/全息 HUD、
3D 变换/参与后期），更"游戏化"。Bevypunk 提供生产级参考架构（状态驱动 UI、导航、世界空间标牌）。

## 原子任务清单（checklist）

### A. 基础集成与验证
- [x] **A1** 添加 bevy_lunex 0.7 依赖，注册插件，最小界面编译运行（cargo check/test/build/冒烟）
  - `bevy_lunex 0.7`（bevy ^0.19 ✓）+ `bevy_rich_text3d 0.7`（text3d 特性）
  - bevy 增加 `picking` 特性（lunex 交互基于 bevy_picking；DefaultPlugins 据此注册 DefaultPickingPlugins）
  - `src/ui/lunex.rs`：独立 `Camera2d`（透明清屏、order=1）+ `UiSourceCamera::<0>`；
    `UiLayoutRoot::new_2d()` + `UiFetchFromCamera::<0>`；顶部标题横幅
    （`UiMeshPlane2d` + 需**自行提供** `MeshMaterial2d`，lunex 只重建 Mesh2d 几何）
  - 冒烟验证（组件探针）：root dimension=1280×720 正确注入；横幅 mesh+material 产出
- [x] **A2** **CJK 中文字体验证**（最高风险项）— ✅ 通过
  - Text2d（原生 Bevy 文本管线）："黄鹤楼 · 筑梦江城" 排布 372×53 ✓
  - Text3d（bevy_rich_text3d / cosmic-text）：网格产出 dim=7.59×0.9 ✓
    （实体须自带 `Mesh3d` + 引用 `TextAtlas::DEFAULT_IMAGE` 的 `MeshMaterial3d`，
    否则 get_mesh 直接跳过 → 无网格；子集字体经 `LoadFonts.font_paths` 注入）
  - 复现：`PHOENIX_TEXT3D_PROBE=1`（生成探针文本）+ `PHOENIX_UI_PROBE=1`（打印诊断）
- [x] **A3** 输入/交互模型验证：lunex 0.7 基于 bevy_picking（`Pointer<Click/Over/Out>`
  观察者、`Pickable`、lunex 自带 2D picking 后端）。已实现「UI 悬停 → 3D 放置拦截」：
  `placement::handle_place_and_undo` 读取 `HoverMap`（PreUpdate 更新），指针落在任一
  `With<UiLayout>` 实体上时跳过放置/拆除（A3 门控；与轨道相机拖拽的完整共存回归见 D2）
- [x] **A4** UI 架构骨架：`LunexTheme` 资源（banner/panel/行/文字/强调色，杜绝魔法数字）+
  层级约定（HUD 层 / 面板层，重叠时用 UiDepth）；持久 UI 根（本作单屏游戏，
  不引入 Bevypunk 的 OnEnter/OnExit 场景切换，GameState 资源已覆盖状态需求）

### B. 面板逐 Tab 迁移（保留 egui 并行，feature 切换）
- [x] **B1** 积木面板（35 种列表 / 选择高亮 / 点击选中）→ lunex 滚动列表
  - 左侧面板（`PHOENIX_LUNEX_PALETTE=1` 启用，与 egui 面板并存阶段默认关）
  - 35 行：色块 + 中文名；悬停高亮（UiHover）+ 选中高亮（UiSelected 状态，
    非 prelude 导出，需 `bevy_lunex::UiSelected` 显式导入）
  - 点击选中：`.observe(On<Pointer<Click>>)` 直接写 `BlockLibrary.current`
    （与键盘 1-9/Q/E 同源）；选中同步系统监听 `Res<BlockLibrary>.is_changed()`
  - 滚轮滚动：仅指针悬停于行上时生效；窗口外的行置 `Visibility::Hidden`
    （lunex 不裁剪子节点，越界行必须显式隐藏；首次运行即应用初始窗口）
  - 冒烟验证：35 行全部产出几何+材质；初始可见 14 行；教程选中目标行高亮正确
- [x] **B2** 蓝图模式区：完成度进度条 + 蓝图主题下拉
  - 进度条：填充宽度 = `Blueprint.completion`（`Res<Blueprint>.is_changed()` 门控，
    变更时改 UiLayout 尺寸 + 「42%」文本）
  - 主题下拉：按钮（全宽，悬停高亮）开合 `ThemeMenuOpen` → 行可见性；
    行点击 → `select_blueprint` + 销毁旧幽灵蓝图（与 egui 面板同一流程）
  - 冒烟验证：按钮显示当前主题名；进度 0%；下拉默认收起
- [x] **B3** 挑战区：挑战选择下拉 + 状态/倒计时/材料 + 开始/重试按钮
  - 选择下拉（同 B2 模式：按钮开合 → 行可见性 → 行点击 select_challenge）
  - 状态行 + 按钮文本随 `Challenge` 变化同步（进行中每帧 tick 更新；
    双 `Query<&mut Text2d>` 需 ParamSet 规避 B0001 冲突）
  - 开始/重试按钮：非 Active 时点击 → `start_challenge`（清空世界，与 C 键同一流程）
  - 冒烟验证：名称/状态（未开始）/按钮（开始）正确；无 panic
- [x] **B4** 存档 Tab：文件列表 / 加载 / 保存 / 分享 / JSON / 路径输入
  - **Tab 架构**（A4 落地）：`LunexTab` 资源 + `TabButton`/`TabBlocksRoot`/`TabSavesRoot`
    标记；`lunex_tab_sync` 切换内容可见性 + 按钮高亮（UiSelected）
  - 存档动作提取为 save/mod.rs 公共函数（save_slot/export_json/share_export/
    load_slot/import_latest/save_file_list），F5-F9 快捷键与面板按钮共用单一来源
  - 自建文本输入框（lunex 无内置）：`MessageReader<KeyboardInput>` 接收字符/
    退格/Esc/Enter，聚焦态高亮；路径导入走 `load_save_from_path` + `import_save`
  - 文件列表每 1 秒刷新（仅存档 Tab 激活时），显示名称/积木数/时间
  - 冒烟验证：4 个 Tab 渲染且 Blocks 高亮；存档内容默认隐藏；无 panic
- [x] **B5** 图鉴 Tab（35 条目解锁状态 + 文化描述）
  - 35 行（解锁 ✓ / 未解锁 🔒，暗色区分）+ 滚轮滚动（同积木面板模式）
  - 点击行 → 详情区显示名称/解锁状态/文化描述（`CodexSelected` 资源 + 同步系统）
  - 解锁状态随 `Collection` 变化刷新（行文本 + UiColor 整体替换，字段私有不可改）
  - 修复：`lunex_tab_sync` 无过滤会隐藏**所有** Visibility 实体（含列表行）——
    加 `Or<(With<TabBlocksRoot>, With<TabSavesRoot>, With<TabCodexRoot>)>` 只动内容根
- [x] **B6** 成就 Tab（5 项状态）
  - 5 张成就卡片（名称/描述/✓ 或 🔒），随 `Collection` 解锁状态刷新
    （解锁金色、未解锁常规色；UiColor 整体替换）
  - 冒烟验证：5 卡渲染、初始全锁定（🔒）、Tab 隐藏态正确
- [x] **B7** 知识卡片（PRD §3.3 智能提示）
  - toast 卡片（横幅下方，Rl(44)×Rh(9.5)），`KnowledgeHints.card` 变化 → 显隐 + 标题/描述
  - 双 `Query<&mut Text2d>` 用 ParamSet 规避 B0001
  - 冒烟验证：初始隐藏；停顿 >12s 后出现并显示「📖 大台基」等提示
- [x] **B8** 蓝图透明度滑杆 + 其余小控件
  - 自建滑杆（lunex 无内置）：轨道 + 填充 + 滑块；`Pointer<Press/Move/Release>` +
    `OpacityDragging` 资源驱动（命中世界 x → alpha 0.1..0.8）
  - alpha 变化 → 填充/滑块位置同步 + 幽灵蓝图材质透明度（与 egui 面板同效）
  - 冒烟验证：初始填充 35.7%（= 默认 alpha 0.35）；无 panic

> **B 阶段全部完成（B1-B8）**：积木/蓝图/挑战/存档/图鉴/成就/知识卡片/滑杆均已 lunex 化，
> 进入 C 阶段（世界空间 UI）。

### C. 世界空间 UI（Lunex 差异化价值）
- [x] **C1** 匾额「黄鹤楼」世界空间文字（Text2d 直挂 → bevy_rich_text3d Text3d）
  - `reconcile_plaque_text`：放置匾额时挂 Text3d（Mesh3d + 引用 TextAtlas::DEFAULT_IMAGE
    的材质；CJK 子集字体经 LoadFonts 注入）；删除原 Text2d 与 RenderLayers 规避
  - 验证：`PHOENIX_LOAD=saves/verify_plaque.ptw`（新测试生成）→ 匾额实体
    Text3d 网格产出（mesh=true）；单测 +1（53/53）
- [x] **C2** HUD 全息化评估（FPS/模式行世界空间 or 屏幕层——按需）
  - **结论：保持屏幕层。** 本作 HUD 信息（模式/完成度/挑战/FPS）为功能性读数，
    世界空间全息投影（Bevypunk 风格）对玩法清晰度无增益、徒增遮挡与 billboard 复杂度；
    标题横幅 + 面板已是 lunex 屏幕层，FPS/提示行经 bevy_ui 由同一 UI 相机绘制。
    若未来要做氛围化开场/菜单可再引入。
- [x] **C2** HUD 全息化评估（FPS/模式行世界空间 or 屏幕层——按需）
  - **结论：保持屏幕层。** 本作 HUD 信息（模式/完成度/挑战/FPS）为功能性读数，
    世界空间全息投影（Bevypunk 风格）对玩法清晰度无增益、徒增遮挡与 billboard 复杂度；
    标题横幅 + 面板已是 lunex 屏幕层，FPS/提示行经 bevy_ui 由同一 UI 相机绘制。
    若未来要做氛围化开场/菜单可再引入。

### D. 收尾
- [x] **D1** 移除 bevy_egui 依赖与代码
  - 删除 bevy_egui 依赖、EguiPlugin、block_panel.rs（4 Tab 全部已 lunex 化）
  - lunex 面板默认启用（移除 PHOENIX_LUNEX_PALETTE 门控）
  - 冒烟验证：无 egui 启动正常，Tab/图鉴/成就/积木面板全部产出，零 panic
- [x] **D2** 输入共存回归（放置/轨道/UI 点击互不冲突）— 代码级验证完成
  - 放置/拆除：HoverMap 门控（A3），指针悬停于任意 UiLayout 节点时跳过；
    Tab 切换后隐藏内容自动退出 picking（Visibility::Hidden → ViewVisibility false）
  - UI 点击：行/按钮/下拉/滑杆均走 bevy_picking 观察者，与 3D 点击互斥
  - 滚轮：列表滚动仅悬停于行上时生效（不干扰相机缩放）
  - ⏳ 手感回归（拖拽/滚动方向/点击灵敏度）待真人 playtest（见 ACCEPTANCE）
- [x] **D3** 全量回归：cargo test + 冒烟 + 性能对比
  - 测试 53/53 通过；冒烟（默认启动 + PHOENIX_LOAD 存档 + PHOENIX_UI_PROBE 探针）零 panic
  - 截图：离屏 CaptureCamera 只含 3D 场景（UI 不混入，截图干净，属预期）
  - 性能：空闲世界 + lunex 面板实测 **~58 FPS**（此前带 egui 面板 ~36）——
    retained 布局开销显著更小（README/RELEASE_READINESS 已同步）
- [x] **D4** 文档同步（README/RELEASE_READINESS/BACKLOG）+ 提交（全部提交完成）

## 风险与决策点
1. **CJK 文本**（A2）— lunex 基于 Bevy 文本管线，需实证 → ✅ 已实证（Text2d 与 Text3d 均通过）
2. **控件自建**：lunex 是布局引擎，下拉/滑杆/文本输入需自建或借用 Bevypunk 模式
3. **输入共存**（A3/D2）— lunex 交互与 3D 点击、轨道相机拖拽的协调
4. **性能对比**（D3）— retained 模式应优于 egui 每帧重画，实测确认
5. 迁移期间 egui 并行保留（feature 开关），B 完成后一键切换，风险可控

## 修复记录：B5 图鉴行整屏色块 + 下拉深度（2026-09-03）

**症状**：画面闪烁 / 大片同色覆盖看不清 / UI 互相压叠（图鉴 Tab 隐藏内容泄漏成
整屏色块）。

**根因 1（整屏色块/闪烁）**：图鉴（codex）行把 `UiMeshPlane2d` 与 `Text2d` 放在**同一实体**。
lunex 的 `system_text_size_from_dimension` 会按文本排布缩放实体 `Transform`——
文本与 mesh 同实体时，行 mesh 被放大成整屏大色块（锁定行 = text_dim 色、解锁行 =
text_main 色），覆盖 3D 场景并随布局抖动 → 观感「闪烁 + 看不清 + 字体异常」。
**修复**：文本改为行的**独立子节点**（与积木面板行同构），状态同步改查子节点
`CodexNameText`。修复后离屏捕获：UI 布局正确、两帧 0.00% 差异（稳定）。

**根因 2（下拉叠在列表上）**：主题/挑战下拉行与积木列表行同为 z=3（UiDepth 逐层 +1），
同深度重叠透明四边形渲染顺序不定。**修复**：下拉行 `UiDepth::Add(2.0)`（z=4>3），
打开时浮于列表之上。

**附带**：UI 相机 `Msaa::Off`（2D 层无需抗锯齿；与 3D 相机共享窗口目标时 MSAA
跨相机 load/resolve 在 Metal 上不稳，先关掉 2D 侧规避）。

**诊断方法**：本机窗口截图读回全黑（B-21 平台性 bug）+ 常开离屏相机与 Screenshot
读回竞态 → 改用**一次性激活**的离屏 2D 相机（截图帧激活/次帧停用）+ 最小 Sprite /
Mesh2d 隔离测试确认 2D 管线本身正常，再逐块二分定位到 codex 行。



- **lunex 0.7 API 要点**（对照 Bevypunk 验证）：
  - 2D UI 相机：独立 `Camera2d` + `Camera { clear_color: ClearColorConfig::None, order: 1 }`
    + `UiSourceCamera::<0>`，`Transform::from_translation(Vec3::Z * 1000.0)`；
    UI 根实体：`UiLayoutRoot::new_2d()` + `UiFetchFromCamera::<0>`。
    bevy_ui（旧 HUD 提示）会改由 order 最高的主窗口相机绘制（行为不变）。
  - 节点：`UiLayout::window().pos((Rl(x), Rh(y))).size((Rl(w), Rh(h))).anchor(Anchor::TOP_CENTER).pack()`；
    纯色面板 = `UiMeshPlane2d` + `MeshMaterial2d(handle)`（lunex 只写 Mesh2d 几何，
    材质必须自己建；`UiColor` 系统随后按状态着色）+ `UiColor::new(vec![(UiBase::id(), color)])`。
  - 文本：`Text2d::new(...)` + `TextFont` + `UiTextSize::from(Rh(..))` +
    `UiLayout::window().full().pack()`，`Pickable::IGNORE` 防误触。
  - **Text3d（bevy_rich_text3d）必须自带 `Mesh3d` + `MeshMaterial3d`**（引用
    `TextAtlas::DEFAULT_IMAGE`、`AlphaMode::Blend`），否则 `get_mesh` 返回 None、文本静默不渲染。
  - 自定义字体注入：`LoadFonts.font_paths` 在 `UiLunexPlugins` 之前填充
    （Text3dPlugin cleanup 读取；init_resource 不覆盖已存在值）。
  - 相机数量：新增 Camera2d 不参与 `With<Camera3d>` 查询，`camera_sanity_check` 不受影响；
    世界空间 `Text2d`（匾额）只能被 Camera2d 绘制——已置 `RenderLayers::layer(1)`
    保持不渲染现状，C1 用 lunex UiRoot3d/Text3d 重做。
- **A3 门控**：`HoverMap`（bevy_picking，PreUpdate 更新）+ `Query<Entity, With<UiLayout>>`；
  放置/拆除系统在鼠标释放时若指针悬停于 UI 节点则跳过。UI 节点点击事件后续用
  `.observe(|_: On<Pointer<Click>>| ...)` + `hover_set::<Pointer<Over>, true>` 实现。
- **CJK 字形警告**：`ICU4X data error: No segmentation model for Chinese/Japanese`
  是 cosmic-text 分词模型的非致命告警，不影响字形渲染，忽略即可。

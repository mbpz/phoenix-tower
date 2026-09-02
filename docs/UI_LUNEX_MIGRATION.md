# UI 迁移计划：bevy_egui → Bevy-Lunex（参考 Bevypunk）

> 状态：进行中（2026-09）· 目标：用 Bevy-Lunex（retained/ECS 布局引擎）重构游戏 UI 交互，
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
- [ ] **B8** 蓝图透明度滑杆 + 其余小控件

### C. 世界空间 UI（Lunex 差异化价值）
- [ ] **C1** 匾额「黄鹤楼」/ 灯笼等世界空间文字/标牌改造（替代 Text2d 直挂）
- [ ] **C2** HUD 全息化评估（FPS/模式行世界空间 or 屏幕层——按需）

### D. 收尾
- [ ] **D1** 移除 bevy_egui 依赖与代码（在 B 全部完成后 feature 切换验证）
- [ ] **D2** 输入共存回归（放置/轨道/UI 点击互不冲突）
- [ ] **D3** 全量回归：cargo test + 冒烟 + 截图 + 性能对比（egui vs lunex 面板开销）
- [ ] **D4** 文档同步（README/RELEASE_READINESS）+ 提交

## 风险与决策点
1. **CJK 文本**（A2）— lunex 基于 Bevy 文本管线，需实证 → ✅ 已实证（Text2d 与 Text3d 均通过）
2. **控件自建**：lunex 是布局引擎，下拉/滑杆/文本输入需自建或借用 Bevypunk 模式
3. **输入共存**（A3/D2）— lunex 交互与 3D 点击、轨道相机拖拽的协调
4. **性能对比**（D3）— retained 模式应优于 egui 每帧重画，实测确认
5. 迁移期间 egui 并行保留（feature 开关），B 完成后一键切换，风险可控

## A 阶段实现记录（2026-09）

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

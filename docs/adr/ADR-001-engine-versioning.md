# ADR-001：引擎与版本策略

- 状态：已接受（2026-08）
- 关联：docs/PRD.md §4、docs/REVIEW.md 技术核实 #1/#6

## 背景

方案要求 Bevy ≥ 0.19。经 crates.io 核实当前最新稳定为 **0.19.1**（2026-08-13 发布）。

## 决策

1. **锁定 Bevy 0.19 大版本**（`bevy = "0.19"`），不追 minor 升级，升级走官方迁移指南。
2. **代码风格用经典命令式 ECS API**（组件 + 系统 + 资源）。Bevy 0.19 新增场景 DSL（`bsn_list!` / `SceneList`）用于关卡/蓝图数据资产化，项目代码暂不使用，Phase 1 评估是否用于蓝图定义。
3. 渲染特性：使用 Bevy 内置 GPU-driven 能力；实例化渲染在 Phase 2 分块网格化时引入。

## 后果

- 优点：API 稳定可查（本仓库所有 Phase 0 代码均已对照 v0.19.1 官方示例与源码验证）。
- 成本：升级 minor 版本需走迁移指南；DSL 生态未成熟前不依赖。

## 0.19 关键 API 变更备忘（Phase 1 开发者必读）

1. **输入事件机制变更**：`Event`/`EventReader`/`EventWriter` 被 `Message`/`MessageReader`/`MessageWriter` 取代；`EventReader` 已不在 prelude。
2. **鼠标输入简化**：`MouseMotion`/`MouseWheel` 改为逐帧累加资源 `AccumulatedMouseMotion`/`AccumulatedMouseScroll`（`bevy::input::mouse`，需显式导入），官方示例 `examples/input/mouse_input.rs` 即用此法。
3. **文字**：`TextFont.font_size` 类型由 `f32` 改为 `FontSize` 枚举（如 `FontSize::Px(16.0)`）。
4. **灯光明暗**：`GlobalAmbientLight { brightness }` 与 `DirectionalLight { illuminance }` 使用物理单位（lux）；点光用 `intensity`（lumens）。
5. 相机射线：`Camera::viewport_to_world` + `Ray3d::intersect_plane` 保留（Phase 0 已验证）。

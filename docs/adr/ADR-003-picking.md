# ADR-003：拾取 / 射线方案

- 状态：已接受（2026-08）
- 关联：docs/PRD.md §4、docs/REVIEW.md 技术核实 #3/#5（必须修正项）

## 背景

方案原文建议"bevy_mod_picking **或** 内置 picking"。核实结果：**bevy_mod_picking 已停更**（0.20.1，2024-07，不兼容 Bevy 0.15+），Bevy 自 0.15 起内置 `bevy_picking`。方案此项必须修正。

## 决策

1. **统一采用 Bevy 内置 picking**（`bevy::picking`），禁止引入 bevy_mod_picking。
2. **Phase 0 放置链路不依赖 picking 插件**，用纯数学射线：
   `Camera::viewport_to_world` → `Ray3d::intersect_plane(y=0)` → 网格吸附。已验证 v0.19.1 保留这两个 API。
3. 内置 picking 用于 Phase 1+ 的实体级交互（点击选中/删除/旋转积木）；网格级放置始终走数学射线（O(1) 查重，见 ADR-005）。

## 后果

- 优点：少一个停更依赖；数学射线零物理开销、完全确定（可回放）。
- 成本：实体级交互需另行接入内置 picking 的事件与焦点系统（Phase 1 任务）。

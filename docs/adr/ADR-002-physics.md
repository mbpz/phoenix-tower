# ADR-002：物理引擎选型（avian3d）

- 状态：已接受（2026-08）
- 关联：docs/PRD.md §4、docs/REVIEW.md 技术核实 #2、BACKLOG B-16

## 背景

方案二选一：avian3d 或 bevy_rapier3d。两者均为 rapier 内核绑定；avian 是绑定层更名后的官方继续路线（avianphysics/avian，crates.io 最新 0.7.0），bevy_rapier3d 0.36 仍维护但社区重心已迁移。

## 决策

1. **物理引擎采用 avian3d 0.7**，Phase 1 集成，Phase 0 不引入（放置用纯数学射线，见 ADR-003）。
2. 物理只用于**挑战模式的"结构稳定性"变体**与可选掉落效果，不参与蓝图模式的主放置链路（保持确定性、可撤销）。
3. 物理世界的积木 Collider 从积木定义（RON）生成，与渲染网格解耦。

## 后果

- 优点：社区活跃、与 Bevy 版本同步快；玩法差异化（结构稳定性挑战）。
- 成本：确定性存档回放需处理物理随机性（种子化模拟）；Phase 2 评估是否全量启用。

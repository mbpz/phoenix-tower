# ADR-004：游戏状态机

- 状态：已接受（2026-08）
- 关联：src/game_state.rs、docs/PRD.md §2

## 背景

游戏有模式/阶段概念，需要 Bevy `States` 表达；同时 Phase 0 只有最小场景，避免过早引入复杂状态。

## 决策

1. `GameState` 枚举（src/game_state.rs）：
   - `Menu`（主菜单，Phase 1 实现）
   - `Playing`（游戏进行中）
2. Phase 1 起按模式扩展：
   `Menu → BlueprintTutorial / Blueprint / FreeBuild / Challenge`，子状态（如蓝图内的 `Placing/Inspecting`）用 SubStates。
3. 状态迁移规则：教程完成 → 解锁 FreeBuild 与完整 Blueprint；Blueprint 完成 → 进入完成动画（固定观赏视角）→ 回 Menu。

## 后果

- 优点：状态显式化，UI/输入/相机按状态门控，避免全局布尔混乱。
- 成本：Phase 0 的 `Playing` 暂未接入门控，仅为骨架（见代码注释）。

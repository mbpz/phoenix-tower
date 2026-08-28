//! 游戏全局状态机。
//!
//! Phase 0 只注册 Menu / Playing 两个最小状态，尚未接入系统门控；
//! Phase 1 起按状态切换：Menu → BlueprintTutorial / Blueprint / FreeBuild / Challenge。
//! 状态迁移图见 docs/adr/ADR-004-game-states.md。
//! 当前为 Phase 0 骨架，未接入状态迁移，故 allow(dead_code) 直至 Phase 1 接线。

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(dead_code)]
pub enum GameState {
    /// 主菜单（Phase 1 实现）
    #[default]
    Menu,
    /// 游戏进行中
    Playing,
}

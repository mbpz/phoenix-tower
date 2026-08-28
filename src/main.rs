//! 黄鹤楼积木：筑梦江城
//!
//! 当前阶段：Phase 1 起步（B-06 数据驱动积木 + B-13 场景基础）。
//! 任务追踪见 docs/BACKLOG.md，产品方案见 docs/PRD.md。

mod building;
mod camera;
mod game_state;
mod scene;
mod ui;

use bevy::prelude::*;

use building::placement::PlacementPlugin;
use camera::orbit_camera::OrbitCameraPlugin;
use game_state::GameState;
use scene::ScenePlugin;
use ui::hud::HudPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .add_plugins((
            OrbitCameraPlugin,
            PlacementPlugin,
            ScenePlugin,
            HudPlugin,
        ))
        .run();
}

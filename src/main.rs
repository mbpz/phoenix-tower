//! 黄鹤楼积木：筑梦江城
//!
//! 当前阶段：Phase 1 起步（B-06 数据驱动积木 + B-13 场景基础）。
//! 任务追踪见 docs/BACKLOG.md，产品方案见 docs/PRD.md。

mod audio;
mod building;
mod camera;
mod game_state;
mod save;
mod scene;
mod ui;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use audio::AudioPlugin;
use building::challenge::ChallengePlugin;
use building::collection::CollectionPlugin;
use building::placement::PlacementPlugin;
use building::tutorial::TutorialPlugin;
use camera::orbit_camera::OrbitCameraPlugin;
use game_state::GameState;
use save::SavePlugin;
use scene::ScenePlugin;
use ui::block_panel::BlockPanelPlugin;
use ui::hud::HudPlugin;

fn main() {
    // 资产根目录显式锚定到项目根（Bevy 默认以可执行文件目录为基准，
    // 直接运行 target/debug/phoenix-tower 时会找不到 assets/）
    let asset_root = format!("{}/assets", env!("CARGO_MANIFEST_DIR"));
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: asset_root,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .init_state::<GameState>()
        .add_plugins((
            OrbitCameraPlugin,
            PlacementPlugin,
            TutorialPlugin,
            ChallengePlugin,
            CollectionPlugin,
            AudioPlugin,
            ScenePlugin,
            HudPlugin,
            BlockPanelPlugin,
            SavePlugin,
        ))
        .run();
}

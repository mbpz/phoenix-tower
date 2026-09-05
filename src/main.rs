//! 黄鹤楼积木：筑梦江城
//!
//! 当前阶段：Phase 1 起步（B-06 数据驱动积木 + B-13 场景基础）。
//! 任务追踪见 docs/BACKLOG.md，产品方案见 docs/PRD.md。

// Bevy 系统以独立参数表达依赖，参数数量超 lint 阈值是正常形态
#![allow(clippy::too_many_arguments)]

mod audio;
mod building;
mod camera;
mod game_state;
mod i18n;
mod riverside;
mod riverside_environment;
mod save;
mod scene;
mod screenshot;
mod stability;
mod stress;
mod ui;

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

use audio::AudioPlugin;
use building::challenge::ChallengePlugin;
use building::collection::CollectionPlugin;
use building::placement::PlacementPlugin;
use building::tutorial::TutorialPlugin;
use camera::orbit_camera::OrbitCameraPlugin;
use game_state::GameState;
use i18n::I18nPlugin;
use save::SavePlugin;
use scene::ScenePlugin;
use screenshot::ScreenshotPlugin;
use stability::StabilityPlugin;
use stress::StressPlugin;
use ui::hud::HudPlugin;
use ui::lunex::LunexUiPlugin;

fn main() {
    // 资产根目录显式锚定到项目根（Bevy 默认以可执行文件目录为基准，
    // 直接运行 target/debug/phoenix-tower 时会找不到 assets/）
    let asset_root = format!("{}/assets", env!("CARGO_MANIFEST_DIR"));
    let riverside = riverside::RiversideMode::requested(
        std::env::args().any(|arg| arg == "--riverside"),
        std::env::var_os("PHOENIX_LOAD").is_some(),
        std::env::var_os("PHOENIX_STRESS").is_some(),
    );
    App::new()
        .insert_resource(riverside)
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: asset_root,
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
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
            LunexUiPlugin,
            SavePlugin,
            ScreenshotPlugin,
            StabilityPlugin,
            I18nPlugin,
            StressPlugin,
        ))
        .add_plugins(riverside::RiversidePlugin)
        .add_systems(Startup, riverside_environment::setup)
        .run();
}

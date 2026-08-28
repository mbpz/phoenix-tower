//! 黄鹤楼积木：筑梦江城
//!
//! Phase 0 技术验证目标（见 docs/PRD.md §7）：
//! - Bevy 空场景
//! - 基础轨道相机（旋转 / 缩放 / 平移）
//! - 单个可放置立方体积木（射线检测 → 网格吸附 → 生成实体）
//!
//! 操作说明：
//! - 左键点击   ：放置积木（点击，非拖拽）
//! - 左键拖拽   ：旋转视角（轨道）
//! - 右键拖拽   ：平移视角（平移视点）
//! - 滚轮       ：缩放
//! - Backspace  ：撤销最后一次放置

mod building;
mod camera;
mod game_state;

use bevy::prelude::*;

use building::placement::PlacementPlugin;
use camera::orbit_camera::OrbitCameraPlugin;
use game_state::GameState;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .add_plugins((OrbitCameraPlugin, PlacementPlugin))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 300.0,
            ..default()
        })
        .add_systems(Startup, setup_scene)
        .add_systems(Startup, setup_ui_hint)
        .run();
}

/// 基础场景：地面（模拟台基/蛇山基座）+ 太阳光。
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.36, 0.38, 1.0),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::default(),
        Name::new("Ground"),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.6, 0.0)),
        Name::new("Sun"),
    ));
}

/// 屏幕提示（Phase 0 使用英文，避免默认字体缺 CJK 字形；
/// 中文字体资源列为 Phase 1 积木资产任务之一，见 docs/BACKLOG.md）。
fn setup_ui_hint(mut commands: Commands) {
    commands.spawn((
        Text::new(
            "LMB click: place block | LMB drag: orbit | RMB drag: pan\n\
             Scroll: zoom | Backspace: undo last | Esc: quit",
        ),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

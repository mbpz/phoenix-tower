//! 主场景（B-13）：蛇山台地 + 长江水面 + 昼夜切换。
//!
//! 对应 PRD §3.2「固定主场景：蛇山 + 长江远景 + 可切换昼夜/天气」。
//! Phase 2 升级：正式地形高度图、雾、天气系统。

use bevy::prelude::*;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.62, 0.74, 0.89)))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 300.0,
                ..default()
            })
            .insert_resource(SkyState::default())
            .add_systems(Startup, (setup_ground, setup_snake_hill, setup_river, setup_sun))
            .add_systems(Update, day_night_system);
    }
}

// ---------- 地形 ----------

fn setup_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // 青灰地面（蛇山基座所在平原）
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(130.0, 130.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.37, 0.40),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 25.0),
        Name::new("Ground"),
    ));
}

/// 蛇山：三级青石台地堆叠（移出建造区作背景，正式地形 Phase 2 用高度图）。
fn setup_snake_hill(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // (w, d, h, y_center, x_center) —— 山体中心 (-18, 14)，避开建造区
    let layers: [(f32, f32, f32, f32, f32); 3] = [
        (18.0, 18.0, 2.0, 1.0, -18.0),
        (12.0, 12.0, 2.0, 3.0, -18.0),
        (7.0, 7.0, 2.0, 5.0, -18.0),
    ];
    for (i, (w, d, h, y, x)) in layers.iter().enumerate() {
        let shade = 0.34 + i as f32 * 0.04;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(*w, *h, *d))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(shade, shade + 0.02, shade + 0.05),
                perceptual_roughness: 0.9,
                ..default()
            })),
            Transform::from_xyz(*x, *y, 14.0),
            Name::new(format!("SnakeHillLayer{i}")),
        ));
    }
}

/// 长江：远景水面（与平原边缘相接，视觉上形成江岸）。
fn setup_river(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(260.0, 160.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.42, 0.68),
            perceptual_roughness: 0.15,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.05, -60.0),
        Name::new("YangtzeRiver"),
    ));
}

fn setup_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: DAY_SUN,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.6, 0.0)),
        Name::new("Sun"),
    ));
}

// ---------- 昼夜切换 ----------

/// 天空状态：t = 0 白天 → 1 夜晚，按 T 切换并平滑过渡。
#[derive(Resource, Default)]
pub struct SkyState {
    pub night: bool,
    pub t: f32,
}

const DAY_SUN: f32 = 1000.0; // lux（阴天日照）
const NIGHT_SUN: f32 = 60.0;
const DAY_AMB: f32 = 300.0;
const NIGHT_AMB: f32 = 25.0;
const DAY_SKY: [f32; 3] = [0.62, 0.74, 0.89];
const NIGHT_SKY: [f32; 3] = [0.03, 0.05, 0.13];
/// 过渡速度（t/秒），约 2.5 秒完成切换
const TRANSITION_SPEED: f32 = 0.4;

fn day_night_system(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut sky: ResMut<SkyState>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<&mut DirectionalLight>,
) {
    if keys.just_pressed(KeyCode::KeyT) {
        sky.night = !sky.night;
    }

    let target = if sky.night { 1.0 } else { 0.0 };
    let dt = time.delta_secs();
    if sky.t < target {
        sky.t = (sky.t + dt * TRANSITION_SPEED).min(target);
    } else if sky.t > target {
        sky.t = (sky.t - dt * TRANSITION_SPEED).max(target);
    }
    let t = sky.t;
    let lerp = |a: f32, b: f32| a + (b - a) * t;

    if let Ok(mut light) = sun.single_mut() {
        light.illuminance = lerp(DAY_SUN, NIGHT_SUN);
    }
    ambient.brightness = lerp(DAY_AMB, NIGHT_AMB);
    clear.0 = Color::srgb(
        lerp(DAY_SKY[0], NIGHT_SKY[0]),
        lerp(DAY_SKY[1], NIGHT_SKY[1]),
        lerp(DAY_SKY[2], NIGHT_SKY[2]),
    );
}

//! 主场景（B-13）：蛇山台地 + 长江水面 + 昼夜切换。
//!
//! 对应 PRD §3.2「固定主场景：蛇山 + 长江远景 + 可切换昼夜/天气」。
//! Phase 2 升级：正式地形高度图、雾、天气系统。

use crate::ui::input::{shortcuts_allowed, InputOwnership};
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
            .add_systems(
                Startup,
                (setup_ground, setup_snake_hill, setup_river, setup_sun),
            )
            .add_systems(Update, day_night_system);
    }
}

// ---------- 地形 ----------

fn setup_ground(
    mode: Res<crate::riverside::RiversideMode>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if mode.0 {
        return;
    }
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
    mode: Res<crate::riverside::RiversideMode>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if mode.0 {
        return;
    }
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
    mode: Res<crate::riverside::RiversideMode>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if mode.0 {
        return;
    }
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

fn setup_sun(mut commands: Commands, mode: Res<crate::riverside::RiversideMode>) {
    commands.spawn((
        DirectionalLight {
            illuminance: if mode.0 { 5000.0 } else { DAY_SUN },
            color: if mode.0 {
                Color::srgb(1.0, 0.90, 0.78)
            } else {
                Color::WHITE
            },
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
    ownership: Option<Res<InputOwnership>>,
    mode: Res<crate::riverside::RiversideMode>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut sky: ResMut<SkyState>,
    clear: ResMut<ClearColor>,
    ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<&mut DirectionalLight>,
    render: Res<crate::building::placement::BlockRenderAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lantern_lights: Query<&mut PointLight, With<crate::building::placement::LanternLight>>,
    mut fog: Query<&mut DistanceFog>,
) {
    if shortcuts_allowed(&keys, ownership.as_deref()) && keys.just_pressed(KeyCode::KeyT) {
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

    if let Ok(light) = sun.single_mut() {
        light
            .map_unchanged(|light| &mut light.illuminance)
            .set_if_neq(lerp(if mode.0 { 5000.0 } else { DAY_SUN }, NIGHT_SUN));
    }
    ambient
        .map_unchanged(|ambient| &mut ambient.brightness)
        .set_if_neq(lerp(if mode.0 { 450.0 } else { DAY_AMB }, NIGHT_AMB));
    clear
        .map_unchanged(|clear| &mut clear.0)
        .set_if_neq(Color::srgb(
            lerp(DAY_SKY[0], NIGHT_SKY[0]),
            lerp(DAY_SKY[1], NIGHT_SKY[1]),
            lerp(DAY_SKY[2], NIGHT_SKY[2]),
        ));

    // 夜景辉光：灯笼（暖红）与宝顶（暖金）随入夜增强
    let glow: &[(&str, [f32; 3])] = &[
        ("denglong", [1.0, 0.32, 0.18]),
        ("baoding", [1.0, 0.78, 0.3]),
    ];
    for (id, [r, g, b]) in glow {
        if let Some((_, handle)) = render.per_def.get(*id) {
            if let Some(mut m) = materials.get_mut(handle) {
                let emissive = LinearRgba::new(r * t, g * t, b * t, 0.0);
                // AssetMut records a modification on mutable dereference.
                if m.emissive != emissive {
                    m.emissive = emissive;
                }
            }
        }
    }
    // 灯笼点光源强度随入夜增强
    for light in &mut lantern_lights {
        light
            .map_unchanged(|light| &mut light.intensity)
            .set_if_neq(t * 350.0);
    }
    // 雾色昼夜联动：白天浅蓝薄雾 → 夜晚深蓝
    const DAY_FOG: [f32; 3] = [0.78, 0.82, 0.9];
    const NIGHT_FOG: [f32; 3] = [0.05, 0.08, 0.16];
    for f in &mut fog {
        f.map_unchanged(|fog| &mut fog.color)
            .set_if_neq(Color::srgb(
                lerp(DAY_FOG[0], NIGHT_FOG[0]),
                lerp(DAY_FOG[1], NIGHT_FOG[1]),
                lerp(DAY_FOG[2], NIGHT_FOG[2]),
            ));
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use crate::building::placement::BlockRenderAssets;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct ChangedEnvironment(usize);

    fn watch_changes(
        mut count: ResMut<ChangedEnvironment>,
        sky: Res<SkyState>,
        clear: Res<ClearColor>,
        ambient: Res<GlobalAmbientLight>,
        lights: Query<(), Changed<DirectionalLight>>,
        fog: Query<(), Changed<DistanceFog>>,
    ) {
        count.0 = usize::from(sky.is_changed())
            + usize::from(clear.is_changed())
            + usize::from(ambient.is_changed())
            + lights.iter().count()
            + fog.iter().count();
    }

    fn scene_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Time>()
            .init_resource::<SkyState>()
            .init_resource::<ClearColor>()
            .init_resource::<GlobalAmbientLight>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<ChangedEnvironment>()
            .insert_resource(crate::riverside::RiversideMode(false))
            .insert_resource(BlockRenderAssets {
                per_def: Default::default(),
                blueprint_materials: Default::default(),
                ghost_material: default(),
                ghost_bad_material: default(),
                blueprint_unit_mesh: default(),
            })
            .add_systems(Update, (day_night_system, watch_changes).chain());
        app.world_mut().spawn(DirectionalLight::default());
        app.world_mut().spawn(DistanceFog::default());
        app
    }

    #[test]
    fn settled_environment_does_not_dirty_render_state_every_frame() {
        let mut app = scene_app();
        app.update();
        app.update();
        assert_eq!(app.world().resource::<ChangedEnvironment>().0, 0);
    }

    #[test]
    fn night_transition_and_new_lights_still_update_after_settling() {
        let mut app = scene_app();
        app.update();
        app.world_mut().resource_mut::<SkyState>().night = true;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(1));
        app.update();
        assert!((app.world().resource::<SkyState>().t - 0.4).abs() < 1e-5);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<SkyState>().t, 1.0);
        app.update();
        assert_eq!(app.world().resource::<ChangedEnvironment>().0, 0);
        let lamp = app
            .world_mut()
            .spawn((
                PointLight::default(),
                crate::building::placement::LanternLight,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<PointLight>(lamp).unwrap().intensity,
            350.0
        );
    }
}

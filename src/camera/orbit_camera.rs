//! 轨道相机：围绕目标点旋转 / 缩放 / 平移。
//!
//! 交互约定（与方案 §3.2 相机系统对应）：
//! - 左键拖拽：轨道旋转（yaw / pitch）
//! - 右键拖拽：平移视点（target 在相机平面内移动）
//! - 滚轮：指数缩放距离
//!
//! 参数均收敛在有界范围内，保证古建对称美学下视角始终稳定。

use crate::ui::input::InputOwnership;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

pub struct OrbitCameraPlugin;

impl Plugin for OrbitCameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OrbitCamera::default())
            .add_systems(Startup, spawn_orbit_camera)
            .add_systems(
                Update,
                (
                    camera_sanity_check,
                    completion_autopilot_system,
                    overview_shortcut,
                    orbit_camera_system,
                )
                    .chain(),
            );
    }
}

/// 护栏：主相机（排除截图相机）必须恰好 1 个。
/// 多个 Camera3d 会让 `.single()` 静默失效（B-21 曾因此导致鼠标交互全挂），
/// 启动即响亮报错以便及时发现。
fn camera_sanity_check(
    mut done: Local<bool>,
    cameras: Query<(), (With<Camera3d>, Without<crate::screenshot::CaptureCamera>)>,
) {
    if *done {
        return;
    }
    *done = true;
    let n = cameras.iter().count();
    if n != 1 {
        error!("主相机数量异常：{n}（应为 1；多相机会导致 .single() 查询静默失效）");
    }
}

/// 观赏自动驾驶目标（PRD §3.2：完成时自动切换固定观赏视角）。
#[derive(Clone, Copy, Debug)]
pub struct AutoPilot {
    pub goal_yaw: f32,
    pub goal_pitch: f32,
    pub goal_distance: f32,
}

/// 轨道相机参数（资源，而非实体组件：单相机、跨系统共享）。
#[derive(Resource)]
pub struct OrbitCamera {
    /// 视点目标（look_at 焦点）
    pub target: Vec3,
    /// 水平方位角（弧度）
    pub yaw: f32,
    /// 俯仰角（弧度，受 pitch 上下限约束）
    pub pitch: f32,
    /// 相机到目标的距离
    pub distance: f32,
    /// 观赏自动驾驶（完成时启用；玩家输入即接管）
    pub autopilot: Option<AutoPilot>,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.6,
            pitch: 0.55,
            distance: 28.0,
            autopilot: None,
        }
    }
}

const PITCH_MIN: f32 = 0.05;
const PITCH_MAX: f32 = 1.55; // ~ 89°
const DIST_MIN: f32 = 4.0;
const DIST_MAX: f32 = 160.0;
/// 旋转灵敏度（弧度 / 像素）
const ORBIT_SPEED: f32 = 0.006;
/// 平移灵敏度（随距离缩放，保证手感一致）
const PAN_SPEED: f32 = 0.0016;
/// 缩放灵敏度（指数因子 / 滚轮格）
const ZOOM_SPEED: f32 = 0.09;

impl OrbitCamera {
    /// 由轨道参数计算相机位置。
    fn position(&self) -> Vec3 {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let dir = Vec3::new(
            cos_pitch * self.yaw.sin(),
            sin_pitch,
            cos_pitch * self.yaw.cos(),
        );
        self.target + dir * self.distance
    }
}

fn frame_cells(orbit: &mut OrbitCamera, cells: &[IVec3], aspect: f32) {
    if cells.is_empty() {
        return;
    }
    let min = cells
        .iter()
        .fold(Vec3::splat(f32::INFINITY), |bound, cell| {
            bound.min(cell.as_vec3())
        })
        + Vec3::new(-0.5, 0.0, -0.5);
    let max = cells
        .iter()
        .fold(Vec3::splat(f32::NEG_INFINITY), |bound, cell| {
            bound.max(cell.as_vec3())
        })
        + Vec3::new(0.5, 1.0, 0.5);
    orbit.target = (min + max) * 0.5;
    let tan_half_fov = (PerspectiveProjection::default().fov * 0.5).tan();
    let rotation = Transform::from_translation(orbit.position())
        .looking_at(orbit.target, Vec3::Y)
        .rotation
        .inverse();
    let mut distance = DIST_MIN;
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                let p = rotation * (Vec3::new(x, y, z) - orbit.target);
                // Reserve the top quarter for title/knowledge cards and the
                // lower edge for hints, rather than merely fitting the lens.
                let vertical_margin = if p.y > 0.0 { 0.5 } else { 0.75 };
                distance = distance
                    .max(p.z + p.y.abs() / (tan_half_fov * vertical_margin))
                    .max(p.z + p.x.abs() / (tan_half_fov * aspect * 0.8));
            }
        }
    }
    orbit.distance = (distance * 1.03).clamp(DIST_MIN, DIST_MAX);
    orbit.autopilot = None;
}

fn overview_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    input: Option<Res<InputOwnership>>,
    blueprint: Res<crate::building::blueprint::Blueprint>,
    mut orbit: ResMut<OrbitCamera>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if crate::ui::input::shortcuts_allowed(&keys, input.as_deref())
        && keys.just_pressed(KeyCode::Home)
    {
        let aspect = windows
            .single()
            .ok()
            .map(|w| w.width() / w.height().max(1.0))
            .unwrap_or(16.0 / 9.0);
        frame_cells(&mut orbit, &blueprint.cell_list, aspect);
    }
}

fn spawn_orbit_camera(
    mut commands: Commands,
    mut orbit: ResMut<OrbitCamera>,
    blueprint: Option<Res<crate::building::blueprint::Blueprint>>,
    tutorial: Option<Res<crate::building::tutorial::Tutorial>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let aspect = windows
        .single()
        .ok()
        .map(|w| w.width() / w.height().max(1.0))
        .unwrap_or(16.0 / 9.0);
    // First-time builders work on the first tier, not a tiny full-tower silhouette.
    // Startup only: subsequent pan/zoom remains under player control.
    if let Some(tutorial) = tutorial.filter(|t| t.active) {
        let cells: Vec<_> = tutorial
            .steps
            .iter()
            .flat_map(|step| step.cells.iter().copied())
            .collect();
        frame_cells(&mut orbit, &cells, aspect);
    } else if let Some(blueprint) = blueprint {
        frame_cells(&mut orbit, &blueprint.cell_list, aspect);
    }
    commands.spawn((
        Camera3d::default(),
        // Bevy keys shared color textures by MSAA as well as render target.
        // Match the 2D overlay, otherwise its separate texture hides the scene.
        Msaa::Off,
        Transform::from_translation(orbit.position()).looking_at(orbit.target, Vec3::Y),
        // 距离雾（场景纵深与昼夜氛围；颜色由昼夜系统驱动）
        DistanceFog {
            color: Color::srgb(0.78, 0.82, 0.9),
            falloff: FogFalloff::Linear {
                start: 40.0,
                end: 160.0,
            },
            ..default()
        },
        Name::new("OrbitCamera"),
    ));
}

fn orbit_camera_system(
    ownership: Option<Res<InputOwnership>>,
    mut orbit: ResMut<OrbitCamera>,
    // 排除离屏截图相机（B-21 引入的第二个 Camera3d），否则 single_mut 失效
    mut camera_query: Query<
        &mut Transform,
        (With<Camera3d>, Without<crate::screenshot::CaptureCamera>),
    >,
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
) {
    // Bevy 0.19：鼠标位移/滚轮为逐帧累加资源（每帧自动清零）
    let drag_delta = mouse_motion.delta;
    let orbit_drag = mouse_buttons.pressed(MouseButton::Left)
        && ownership
            .as_deref()
            .is_none_or(InputOwnership::orbit_allowed);
    let pan_drag = mouse_buttons.pressed(MouseButton::Right)
        && ownership.as_deref().is_none_or(InputOwnership::pan_allowed);
    let scroll = if ownership
        .as_deref()
        .is_none_or(InputOwnership::zoom_allowed)
    {
        // Trackpads report pixels, not wheel lines. Reuse Bevy's approximate
        // conversion instead of applying line sensitivity to every pixel.
        match mouse_scroll.unit {
            MouseScrollUnit::Line => mouse_scroll.delta.y,
            MouseScrollUnit::Pixel => {
                mouse_scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
            }
        }
    } else {
        0.0
    };
    let input_active =
        ((orbit_drag || pan_drag) && drag_delta.length_squared() > 0.0) || scroll != 0.0;

    // 观赏自动驾驶（PRD §3.2）：无输入时朝目标平滑过渡；玩家输入即接管
    if let Some(pilot) = orbit.autopilot {
        if input_active {
            orbit.autopilot = None; // 玩家接管
        } else {
            let (next, done) = autopilot_step(
                (orbit.yaw, orbit.pitch, orbit.distance),
                (pilot.goal_yaw, pilot.goal_pitch, pilot.goal_distance),
                time.delta_secs(),
            );
            orbit.yaw = next.0;
            orbit.pitch = next.1;
            orbit.distance = next.2;
            if done {
                orbit.autopilot = None;
            }
        }
    }

    if orbit_drag {
        orbit.yaw -= drag_delta.x * ORBIT_SPEED;
        orbit.pitch = (orbit.pitch + drag_delta.y * ORBIT_SPEED).clamp(PITCH_MIN, PITCH_MAX);
    }

    if pan_drag {
        // 相机基向量：right / up（随视角变化）
        let forward = (orbit.target - orbit.position()).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let distance = orbit.distance;
        orbit.target += (right * drag_delta.x + up * drag_delta.y) * PAN_SPEED * distance;
    }

    orbit.distance = (orbit.distance * (-scroll * ZOOM_SPEED).exp()).clamp(DIST_MIN, DIST_MAX);

    if let Ok(mut transform) = camera_query.single_mut() {
        transform.translation = orbit.position();
        transform.look_at(orbit.target, Vec3::Y);
    }
}

/// 蓝图完成（≥95%）上升沿：启用观赏视角自动驾驶。
fn completion_autopilot_system(
    blueprint: Res<crate::building::blueprint::Blueprint>,
    mut orbit: ResMut<OrbitCamera>,
    mut prev_completed: Local<bool>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if blueprint.completed && !*prev_completed {
        let riverside = blueprint.def.id == "riverside";
        let mut overview = OrbitCamera {
            yaw: if riverside { 0.68 } else { 0.15 },
            pitch: if riverside { 0.40 } else { 0.55 },
            ..default()
        };
        let aspect = windows
            .single()
            .ok()
            .map(|w| w.width() / w.height().max(1.0))
            .unwrap_or(16.0 / 9.0);
        frame_cells(&mut overview, &blueprint.cell_list, aspect);
        orbit.target = overview.target;
        orbit.autopilot = Some(AutoPilot {
            goal_yaw: overview.yaw,
            goal_pitch: overview.pitch,
            goal_distance: overview.distance,
        });
        info!("🎥 观赏视角已就位（场景拖拽或滚轮接管）");
    }
    *prev_completed = blueprint.completed;
}

/// 自动驾驶单步插值（纯函数，可测试）：返回（新参数，是否到达）。
pub fn autopilot_step(
    current: (f32, f32, f32),
    goal: (f32, f32, f32),
    dt: f32,
) -> ((f32, f32, f32), bool) {
    const YAW_SPEED: f32 = 1.2; // rad/s
    const PITCH_SPEED: f32 = 1.2;
    const DIST_SPEED: f32 = 12.0; // 单位/s
    let yaw = current.0 + (goal.0 - current.0).clamp(-YAW_SPEED * dt, YAW_SPEED * dt);
    let pitch = current.1 + (goal.1 - current.1).clamp(-PITCH_SPEED * dt, PITCH_SPEED * dt);
    let dist = current.2 + (goal.2 - current.2).clamp(-DIST_SPEED * dt, DIST_SPEED * dt);
    let done =
        (yaw - goal.0).abs() < 0.02 && (pitch - goal.1).abs() < 0.02 && (dist - goal.2).abs() < 0.2;
    ((yaw, pitch, dist), done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_overview_recenters_tutorial_and_keeps_tower_in_frame() {
        use bevy::camera::CameraProjection;
        let mut app = App::new();
        let mut bp = crate::building::blueprint::load_blueprint_library().0;
        bp.completed = true;
        let cells = bp.cell_list.clone();
        app.insert_resource(bp)
            .insert_resource(OrbitCamera {
                target: Vec3::Y * 4.0,
                ..default()
            })
            .add_systems(Update, completion_autopilot_system);
        app.update();
        let orbit = app.world().resource::<OrbitCamera>();
        let pilot = orbit.autopilot.unwrap();
        let view_orbit = OrbitCamera {
            target: orbit.target,
            yaw: pilot.goal_yaw,
            pitch: pilot.goal_pitch,
            distance: pilot.goal_distance,
            ..default()
        };
        let view = Transform::from_translation(view_orbit.position())
            .looking_at(view_orbit.target, Vec3::Y)
            .to_matrix()
            .inverse();
        let mut projection = PerspectiveProjection::default();
        projection.update(1280.0, 720.0);
        let clip = projection.get_clip_from_view() * view;
        for cell in cells {
            let ndc = clip.project_point3(cell.as_vec3() + Vec3::Y);
            assert!(
                ndc.y < 0.55 && ndc.y > -0.8 && ndc.x.abs() < 0.8,
                "{cell:?}: {ndc:?}"
            );
        }
        app.world_mut().resource_mut::<OrbitCamera>().target = Vec3::splat(9.0);
        app.update();
        assert_eq!(
            app.world().resource::<OrbitCamera>().target,
            Vec3::splat(9.0)
        );
    }

    #[test]
    fn tutorial_starts_close_enough_to_click_the_platform() {
        let mut app = App::new();
        app.insert_resource(crate::building::blueprint::load_blueprint_library().0)
            .insert_resource(crate::building::tutorial::load_tutorial())
            .init_resource::<OrbitCamera>()
            .add_systems(Startup, spawn_orbit_camera);
        app.update();
        let orbit = app.world().resource::<OrbitCamera>();
        assert!(
            orbit.target.y < 5.0,
            "tutorial must frame the construction tier"
        );
        assert!(orbit.distance < 35.0, "platform must not be a tiny target");
    }

    #[test]
    fn startup_frames_default_blueprint_below_title_with_native_aspects() {
        use bevy::camera::CameraProjection;
        use bevy::window::PrimaryWindow;
        for (width, height) in [(1280.0, 720.0), (900.0, 720.0)] {
            let mut app = App::new();
            let blueprint = crate::building::blueprint::load_blueprint_library().0;
            let cells = blueprint.cell_list.clone();
            app.insert_resource(blueprint)
                .init_resource::<OrbitCamera>()
                .add_systems(Startup, spawn_orbit_camera);
            app.world_mut().spawn((
                Window {
                    resolution: (width as u32, height as u32).into(),
                    ..default()
                },
                PrimaryWindow,
            ));
            app.update();
            let orbit = app.world().resource::<OrbitCamera>();
            let view = Transform::from_translation(orbit.position())
                .looking_at(orbit.target, Vec3::Y)
                .to_matrix()
                .inverse();
            let mut projection = PerspectiveProjection::default();
            projection.update(width, height);
            let clip = projection.get_clip_from_view() * view;
            for cell in &cells {
                for corner in [Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5)] {
                    let ndc = clip.project_point3(cell.as_vec3() + corner);
                    assert!(
                        ndc.x.abs() < 0.8 && ndc.y > -0.8 && ndc.y < 0.55,
                        "{width}x{height}: cell={cell:?}, ndc={ndc:?}"
                    );
                }
            }
            // Startup framing must never reset a later player pan/zoom.
            app.world_mut().resource_mut::<OrbitCamera>().target = Vec3::splat(9.0);
            app.update();
            assert_eq!(
                app.world().resource::<OrbitCamera>().target,
                Vec3::splat(9.0)
            );
        }
    }

    #[test]
    fn scene_camera_uses_same_samples_as_ui_overlay() {
        let mut app = App::new();
        app.init_resource::<OrbitCamera>()
            .add_systems(Startup, spawn_orbit_camera);
        app.update();
        let mut cameras = app.world_mut().query_filtered::<&Msaa, With<Camera3d>>();
        assert_eq!(*cameras.single(app.world()).unwrap(), Msaa::Off);
    }

    #[test]
    fn pixel_scroll_preserves_line_zoom_sensitivity() {
        use bevy::input::mouse::MouseScrollUnit;
        let distance_after = |unit, delta| {
            let mut app = App::new();
            app.init_resource::<OrbitCamera>()
                .init_resource::<Time>()
                .init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<AccumulatedMouseMotion>()
                .insert_resource(AccumulatedMouseScroll {
                    unit,
                    delta: Vec2::new(0.0, delta),
                })
                .add_systems(Update, orbit_camera_system);
            app.update();
            app.world().resource::<OrbitCamera>().distance
        };
        for lines in [-1.0, 1.0] {
            let wheel = distance_after(MouseScrollUnit::Line, lines);
            let touchpad = distance_after(
                MouseScrollUnit::Pixel,
                lines * MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
            );
            assert!((wheel - touchpad).abs() < 0.001,
                "equivalent scroll must not jump to zoom limits: wheel={wheel}, touchpad={touchpad}");
            assert!(touchpad > DIST_MIN && touchpad < DIST_MAX);
        }
    }

    #[test]
    fn ui_pointer_scroll_and_drag_leave_camera_unchanged() {
        use crate::ui::input::InputOwnershipPlugin;
        use bevy::picking::{backend::HitData, hover::HoverMap, pointer::PointerId};
        use bevy::window::PrimaryWindow;
        use bevy_lunex::UiLayout;

        let mut app = App::new();
        app.init_resource::<OrbitCamera>()
            .init_resource::<Time>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<HoverMap>()
            .add_plugins(InputOwnershipPlugin)
            .add_systems(Update, orbit_camera_system);
        let mut window = Window {
            focused: true,
            ..default()
        };
        window.set_cursor_position(Some(Vec2::ZERO));
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        let row = app.world_mut().spawn(UiLayout::window().pack()).id();
        app.world_mut()
            .resource_mut::<HoverMap>()
            .entry(PointerId::Mouse)
            .or_default()
            .insert(row, HitData::new(row, 0.0, None, None));
        let start = {
            let orbit = app.world().resource::<OrbitCamera>();
            (orbit.yaw, orbit.pitch, orbit.distance, orbit.target)
        };
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta
            .y = -2.0;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut().resource_mut::<HoverMap>().clear();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::splat(100.0);
        app.update();
        let orbit = app.world().resource::<OrbitCamera>();
        assert_eq!(
            (orbit.yaw, orbit.pitch, orbit.distance, orbit.target),
            start
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::ZERO;
        app.update();
        assert_ne!(
            app.world().resource::<OrbitCamera>().distance,
            start.2,
            "wheel outside UI must still zoom after the captured gesture ends"
        );
    }

    #[test]
    fn autopilot_converges_to_goal() {
        let start = (0.6, 0.55, 28.0);
        let goal = (0.15, 0.5, 26.0);
        let mut cur = start;
        let mut done = false;
        for _ in 0..600 {
            let (next, d) = autopilot_step(cur, goal, 1.0 / 60.0);
            cur = next;
            done = d;
            if done {
                break;
            }
        }
        assert!(done, "自动驾驶应在有限步内到达目标");
        assert!((cur.0 - goal.0).abs() < 0.03);
        assert!((cur.2 - goal.2).abs() < 0.3);
    }
}

//! 轨道相机：围绕目标点旋转 / 缩放 / 平移。
//!
//! 交互约定（与方案 §3.2 相机系统对应）：
//! - 左键拖拽：轨道旋转（yaw / pitch）
//! - 右键拖拽：平移视点（target 在相机平面内移动）
//! - 滚轮：指数缩放距离
//!
//! 参数均收敛在有界范围内，保证古建对称美学下视角始终稳定。

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
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

fn spawn_orbit_camera(mut commands: Commands, orbit: Res<OrbitCamera>) {
    commands.spawn((
        Camera3d::default(),
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
    let scroll = mouse_scroll.delta.y;
    let input_active = drag_delta.length() > 0.0
        || scroll.abs() > 0.0
        || mouse_buttons.any_pressed([MouseButton::Left, MouseButton::Middle, MouseButton::Right]);

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

    if mouse_buttons.pressed(MouseButton::Left) {
        orbit.yaw -= drag_delta.x * ORBIT_SPEED;
        orbit.pitch = (orbit.pitch + drag_delta.y * ORBIT_SPEED).clamp(PITCH_MIN, PITCH_MAX);
    }

    if mouse_buttons.pressed(MouseButton::Right) {
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
) {
    if blueprint.completed && !*prev_completed {
        orbit.autopilot = Some(AutoPilot {
            goal_yaw: if blueprint.def.id == "riverside" {
                0.68
            } else {
                0.15
            },
            goal_pitch: if blueprint.def.id == "riverside" {
                0.40
            } else {
                0.55
            },
            goal_distance: if blueprint.def.id == "riverside" {
                28.0
            } else {
                40.0
            },
        });
        info!("🎥 观赏视角已就位（移动鼠标接管）");
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

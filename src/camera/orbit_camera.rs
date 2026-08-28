//! 轨道相机：围绕目标点旋转 / 缩放 / 平移。
//!
//! 交互约定（与方案 §3.2 相机系统对应）：
//! - 左键拖拽：轨道旋转（yaw / pitch）
//! - 右键拖拽：平移视点（target 在相机平面内移动）
//! - 滚轮：指数缩放距离
//!
//! 参数均收敛在有界范围内，保证古建对称美学下视角始终稳定。

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;

pub struct OrbitCameraPlugin;

impl Plugin for OrbitCameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OrbitCamera::default())
            .add_systems(Startup, spawn_orbit_camera)
            .add_systems(Update, orbit_camera_system);
    }
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
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.6,
            pitch: 0.55,
            distance: 28.0,
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
        Name::new("OrbitCamera"),
    ));
}

fn orbit_camera_system(
    mut orbit: ResMut<OrbitCamera>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut scroll_events: EventReader<MouseWheel>,
) {
    let mut drag_delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        drag_delta += ev.delta;
    }

    let mut scroll = 0.0;
    for ev in scroll_events.read() {
        scroll += ev.y;
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
        orbit.target += (right * drag_delta.x + up * drag_delta.y) * PAN_SPEED * orbit.distance;
    }

    orbit.distance =
        (orbit.distance * (-scroll * ZOOM_SPEED).exp()).clamp(DIST_MIN, DIST_MAX);

    if let Ok(mut transform) = camera_query.single_mut() {
        transform.translation = orbit.position();
        transform.look_at(orbit.target, Vec3::Y);
    }
}

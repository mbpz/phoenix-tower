//! 积木放置系统（Phase 0 最小实现）。
//!
//! 验证方案 §4「放置逻辑规约」主链路：
//! `射线检测 → 候选网格位置 → 合法性校验（重叠）→ 生成 Entity`。
//!
//! 技术选择（见 docs/adr/ADR-003-picking.md）：
//! - 用纯数学射线 `Camera::viewport_to_world` + `Ray3d::intersect_plane`，
//!   Phase 0 不引入物理引擎；
//! - 网格吸附：1 单位网格，四舍五入取整；
//! - 幽灵预览：半透明绿色立方体实时跟随吸附位置；
//! - 点击 / 拖拽消歧：左键按下后位移 < 阈值视为「点击放置」。
//!
//! Phase 1 升级点：积木定义数据驱动（RON）、Command 模式撤销历史（≥20 步）、
//! 蓝图匹配校验（稀疏格子集合比较）。见 docs/BACKLOG.md。

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub struct PlacementPlugin;

impl Plugin for PlacementPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlacedBlocks::default())
            .insert_resource(ClickState::default())
            .add_systems(Startup, setup_block_assets)
            .add_systems(
                Update,
                (update_ghost_preview, handle_place_and_undo).chain(),
            );
    }
}

/// 网格尺寸：1 单位 = 1 格。积木为 1×1×1 立方体，底面贴地（y = 0）。
const GRID: f32 = 1.0;
/// 点击 / 拖拽判定阈值（逻辑像素）
const CLICK_DRAG_THRESHOLD: f32 = 6.0;

/// 共享积木资产：所有实体复用同一网格与材质，避免每帧/每块重复分配。
#[derive(Resource)]
pub struct BlockAssets {
    pub cube_mesh: Handle<Mesh>,
    pub solid_material: Handle<StandardMaterial>,
    pub ghost_material: Handle<StandardMaterial>,
}

/// 已放置积木（Phase 0 用简单栈实现「撤销最后一次」；
/// Phase 1 替换为 Command 模式操作历史）。
#[derive(Resource, Default)]
pub struct PlacedBlocks {
    pub stack: Vec<Entity>,
}

/// 点击状态：用于点击 vs 拖拽消歧。
#[derive(Resource, Default)]
struct ClickState {
    pressed_at: Option<Vec2>,
    placed_this_press: bool,
}

/// 幽灵预览标记
#[derive(Component)]
pub struct GhostBlock;

/// 已放置积木标记
#[derive(Component)]
pub struct PlacedBlock;

fn setup_block_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(BlockAssets {
        cube_mesh: meshes.add(Cuboid::new(GRID, GRID, GRID)),
        solid_material: materials.add(StandardMaterial {
            // 朱红漆（参考真实黄鹤楼红柱/红墙）
            base_color: Color::srgb_u8(176, 38, 38),
            perceptual_roughness: 0.55,
            ..default()
        }),
        ghost_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.85, 0.45, 0.35),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

/// 鼠标位置 → y=0 地面上的网格单元（IVec3，y 恒为 0）。
/// 返回 None：光标不在窗口内 / 射线未击中地面（如视角朝上）。
fn cursor_grid_cell(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) -> Option<IVec3> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_gt) = cameras.single().ok()?;
    let ray = camera.viewport_to_world(cam_gt, cursor).ok()?;
    let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    let hit = ray.get_point(t);
    Some(snap_to_grid(hit))
}

/// 任意点吸附到网格单元。
fn snap_to_grid(p: Vec3) -> IVec3 {
    IVec3::new(
        (p.x / GRID).round() as i32,
        (p.y / GRID).round() as i32,
        (p.z / GRID).round() as i32,
    )
}

/// 网格单元 → 实体世界坐标（1×1×1 立方体底面贴地，中心 y = 0.5）。
fn cell_world_pos(cell: IVec3) -> Vec3 {
    Vec3::new(cell.x as f32 * GRID, GRID * 0.5, cell.z as f32 * GRID)
}

/// 幽灵预览：光标不在地面时隐藏（移出可视区）；
/// 落入地面时实时吸附到网格并跟随。
fn update_ghost_preview(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut ghost: Query<(Entity, &mut Transform), With<GhostBlock>>,
    block_assets: Res<BlockAssets>,
) {
    let cell = cursor_grid_cell(&windows, &cameras);
    let target = cell.map(cell_world_pos);

    let mut existing = ghost.single_mut().ok();
    match (existing.take(), target) {
        (None, Some(pos)) => {
            commands.spawn((
                Mesh3d(block_assets.cube_mesh.clone()),
                MeshMaterial3d(block_assets.ghost_material.clone()),
                Transform::from_translation(pos),
                GhostBlock,
                Name::new("GhostBlock"),
            ));
        }
        (Some((_entity, mut transform)), Some(pos)) => {
            transform.translation = pos;
        }
        (Some((_entity, mut transform)), None) => {
            transform.translation = Vec3::new(0.0, -1000.0, 0.0);
        }
        (None, None) => {}
    }
}

/// 放置与撤销：
/// - Backspace：撤销最后一次放置
/// - 左键点击（非拖拽）：在幽灵所在网格放置积木（网格被占用时跳过）
fn handle_place_and_undo(
    mut commands: Commands,
    mut click: ResMut<ClickState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    placed: Query<&Transform, (With<PlacedBlock>, Without<GhostBlock>)>,
    block_assets: Res<BlockAssets>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stack: ResMut<PlacedBlocks>,
) {
    // 撤销：移除最后放置的积木
    if keys.just_pressed(KeyCode::Backspace) {
        if let Some(entity) = stack.stack.pop() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let cursor = windows.single().ok().and_then(|w| w.cursor_position());

    if mouse.just_pressed(MouseButton::Left) {
        click.pressed_at = cursor;
        click.placed_this_press = false;
    }

    if mouse.just_released(MouseButton::Left) {
        let dragged = match (click.pressed_at, cursor) {
            (Some(a), Some(b)) => a.distance(b) > CLICK_DRAG_THRESHOLD,
            _ => true,
        };
        if !dragged && !click.placed_this_press {
            if let Some(cell) = cursor_grid_cell(&windows, &cameras) {
                let occupied = placed
                    .iter()
                    .any(|t| snap_to_grid(t.translation) == cell);
                if !occupied {
                    let entity = commands
                        .spawn((
                            Mesh3d(block_assets.cube_mesh.clone()),
                            MeshMaterial3d(block_assets.solid_material.clone()),
                            Transform::from_translation(cell_world_pos(cell)),
                            PlacedBlock,
                            Name::new("PlacedBlock"),
                        ))
                        .id();
                    stack.stack.push(entity);
                    click.placed_this_press = true;
                }
            }
        }
        click.pressed_at = None;
    }
}

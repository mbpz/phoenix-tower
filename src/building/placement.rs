//! 积木放置系统（B-06 数据驱动版）。
//!
//! 验证方案 §4「放置逻辑规约」主链路：
//! `射线检测 → 候选网格位置 → 合法性校验（重叠）→ 生成 Entity`。
//!
//! 本轮升级：
//! - 积木定义数据驱动化（resources/blocks/*.ron，见 block_defs.rs）；
//! - 支持多尺寸积木（占用 w×1×d 格，哈希集合 O(1) 查重，见 ADR-005）；
//! - 幽灵预览跟随当前选中积木的尺寸与位置。
//!
//! 待办（见 BACKLOG）：热重载、蓝图匹配校验、Command 模式撤销历史（≥20 步）。

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::{HashMap, HashSet};

use super::block_defs::{load_block_library, BlockDef, BlockLibrary};

pub struct PlacementPlugin;

impl Plugin for PlacementPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_block_library())
            .insert_resource(PlacedBlocks::default())
            .insert_resource(ClickState::default())
            .add_systems(Startup, setup_block_assets)
            .add_systems(
                Update,
                (select_block, update_ghost_preview, handle_place_and_undo).chain(),
            );
    }
}

/// 网格尺寸：1 单位 = 1 格。
const GRID: f32 = 1.0;
/// 点击 / 拖拽判定阈值（逻辑像素）
const CLICK_DRAG_THRESHOLD: f32 = 6.0;

/// 渲染资产：def.id → (网格, 材质)，全部预生成并复用（实例化思路）。
#[derive(Resource)]
pub struct BlockRenderAssets {
    pub per_def: HashMap<String, (Handle<Mesh>, Handle<StandardMaterial>)>,
    pub ghost_material: Handle<StandardMaterial>,
}

/// 已放置积木：撤销栈 + 占用集合（O(1) 查重，ADR-005）。
#[derive(Resource, Default)]
pub struct PlacedBlocks {
    pub records: Vec<PlacedRecord>,
    pub occupied: HashSet<IVec3>,
}

/// 一条放置记录（撤销栈元素）。
pub struct PlacedRecord {
    pub entity: Entity,
    pub cells: Vec<IVec3>,
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
    library: Res<BlockLibrary>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut per_def = HashMap::new();
    for def in &library.defs {
        let (w, h, d) = (def.size[0] as f32, def.size[1] as f32, def.size[2] as f32);
        let mesh = meshes.add(Cuboid::new(w * GRID, h * GRID, d * GRID));
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(def.color[0], def.color[1], def.color[2], def.color[3]),
            perceptual_roughness: 0.55,
            ..default()
        });
        per_def.insert(def.id.clone(), (mesh, mat));
    }
    commands.insert_resource(BlockRenderAssets {
        per_def,
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

/// 任意点吸附到网格单元（单层贴地放置，y 恒为 0）。
fn snap_to_grid(p: Vec3) -> IVec3 {
    IVec3::new(
        (p.x / GRID).round() as i32,
        0,
        (p.z / GRID).round() as i32,
    )
}

/// 以光标格为中心对齐的多尺寸积木锚点（footprint 左上角格）。
fn anchor_for(center_cell: IVec3, def: &BlockDef) -> IVec3 {
    let (w, d) = (def.size[0] as i32, def.size[2] as i32);
    IVec3::new(center_cell.x - (w - 1) / 2, 0, center_cell.z - (d - 1) / 2)
}

/// 积木 footprint 占用格集合（y 恒为 0）。
fn footprint_cells(anchor: IVec3, def: &BlockDef) -> Vec<IVec3> {
    let (w, d) = (def.size[0] as i32, def.size[2] as i32);
    let mut cells = Vec::with_capacity((w * d) as usize);
    for dx in 0..w {
        for dz in 0..d {
            cells.push(IVec3::new(anchor.x + dx, 0, anchor.z + dz));
        }
    }
    cells
}

/// 积木渲染中心（底面贴地 y=0）。
fn block_center(anchor: IVec3, def: &BlockDef) -> Vec3 {
    let (w, h, d) = (def.size[0] as f32, def.size[1] as f32, def.size[2] as f32);
    Vec3::new(
        (anchor.x as f32 + (w - 1.0) * 0.5) * GRID,
        h * 0.5 * GRID,
        (anchor.z as f32 + (d - 1.0) * 0.5) * GRID,
    )
}

/// 积木选择：数字键 1-9 直接选择；Q/E 循环切换。
fn select_block(keys: Res<ButtonInput<KeyCode>>, mut library: ResMut<BlockLibrary>) {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) && i < library.defs.len() {
            library.current = i;
            return;
        }
    }
    let len = library.defs.len();
    if keys.just_pressed(KeyCode::KeyQ) {
        library.current = (library.current + len - 1) % len;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        library.current = (library.current + 1) % len;
    }
}

/// 幽灵预览：光标不在地面时隐藏；落入地面时吸附并跟随当前积木尺寸。
fn update_ghost_preview(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut ghost: Query<(Entity, &mut Transform, &mut Mesh3d), With<GhostBlock>>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
) {
    let def = library.current_def();
    let (mesh_handle, _) = &render.per_def[&def.id];
    let cell = cursor_grid_cell(&windows, &cameras);
    let anchor = cell.map(|c| anchor_for(c, def));
    let target = anchor.map(|a| block_center(a, def));

    let mut existing = ghost.single_mut().ok();
    match (existing.take(), target) {
        (None, Some(pos)) => {
            commands.spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(render.ghost_material.clone()),
                Transform::from_translation(pos),
                GhostBlock,
                Name::new("GhostBlock"),
            ));
        }
        (Some((_e, mut transform, mut mesh)), Some(pos)) => {
            transform.translation = pos;
            // 切换积木时同步幽灵网格
            if mesh.0 != *mesh_handle {
                mesh.0 = mesh_handle.clone();
            }
        }
        (Some((_e, mut transform, _mesh)), None) => {
            transform.translation = Vec3::new(0.0, -1000.0, 0.0);
        }
        (None, None) => {}
    }
}

/// 放置与撤销：
/// - Backspace：撤销最后一次放置
/// - 左键点击（非拖拽）：在幽灵所在网格放置当前积木（footprint 被占用时跳过）
fn handle_place_and_undo(
    mut commands: Commands,
    mut click: ResMut<ClickState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stack: ResMut<PlacedBlocks>,
) {
    // 撤销：移除最后放置的积木并释放占用格
    if keys.just_pressed(KeyCode::Backspace) {
        if let Some(record) = stack.records.pop() {
            for c in &record.cells {
                stack.occupied.remove(c);
            }
            commands.entity(record.entity).despawn();
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
                let def = library.current_def();
                let anchor = anchor_for(cell, def);
                let cells = footprint_cells(anchor, def);
                if cells.iter().all(|c| !stack.occupied.contains(c)) {
                    let (mesh, mat) = render.per_def.get(&def.id).expect("积木资产应已预生成");
                    let entity = commands
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(mat.clone()),
                            Transform::from_translation(block_center(anchor, def)),
                            PlacedBlock,
                            Name::new(format!("Block:{}", def.id)),
                        ))
                        .id();
                    for c in &cells {
                        stack.occupied.insert(*c);
                    }
                    stack.records.push(PlacedRecord { entity, cells });
                    click.placed_this_press = true;
                }
            }
        }
        click.pressed_at = None;
    }
}

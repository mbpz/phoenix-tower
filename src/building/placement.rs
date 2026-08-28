//! 积木放置系统（B-08 三维版 + 蓝图模式）。
//!
//! 主链路：`射线检测 → 候选网格位置 → 合法性校验（占用/蓝图匹配）→ 生成 Entity`。
//!
//! - 三维堆叠：光标列上按「列顶高度」落位，可向上建造多层；
//! - 蓝图模式（M 键）：幽灵蓝图 + 严格吸附——仅当放置块 footprint 与蓝图
//!   期望完全一致才允许放置（幽灵红/绿反馈），完成度实时计算（ADR-005）；
//! - 撤销栈保留（Phase 1 完整版替换为 Command 模式 ≥20 步）。

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::{HashMap, HashSet};

use super::block_defs::{load_block_library, BlockDef, BlockLibrary};
use super::blueprint::{
    compute_completion, footprint_matches, load_blueprint_library, Blueprint, BlueprintGhost,
};
use super::challenge::Challenge;
use crate::audio::{play_placement_sound, sound_kind_for, AudioAssets};
use crate::stability::RemoveMode;

pub struct PlacementPlugin;

impl Plugin for PlacementPlugin {
    fn build(&self, app: &mut App) {
        let (blueprint, blueprint_library) = load_blueprint_library();
        app.insert_resource(load_block_library())
            .insert_resource(blueprint)
            .insert_resource(blueprint_library)
            .insert_resource(PlacedBlocks::default())
            .insert_resource(ClickState::default())
            .insert_resource(BlueprintAlpha::default())
            .add_systems(Startup, setup_block_assets)
            .add_systems(
                Update,
                (
                    toggle_blueprint,
                    reconcile_blueprint_ghosts,
                    select_block,
                    update_ghost_preview,
                    handle_place_and_undo,
                )
                    .chain(),
            );
    }
}

/// 网格尺寸：1 单位 = 1 格。
const GRID: f32 = 1.0;
/// 点击 / 拖拽判定阈值（逻辑像素；10px 兼容触控板按下时的微动）
const CLICK_DRAG_THRESHOLD: f32 = 10.0;
/// 蓝图模式 y 扫描上限（防失控循环）
const MAX_BLUEPRINT_Y: i32 = 64;
/// 撤销历史上限（PRD §3.2：撤销/重做至少 20 步）
const MAX_HISTORY: usize = 20;

/// 幽灵蓝图透明度（B-10 打磨：面板滑杆实时调节）。
#[derive(Resource)]
pub struct BlueprintAlpha {
    pub value: f32,
}

impl Default for BlueprintAlpha {
    fn default() -> Self {
        Self { value: 0.35 }
    }
}

/// 渲染资产：def.id → (网格, 材质)，全部预生成并复用（实例化思路）。
#[derive(Resource)]
pub struct BlockRenderAssets {
    pub per_def: HashMap<String, (Handle<Mesh>, Handle<StandardMaterial>)>,
    /// 幽灵（可放置，绿）
    pub ghost_material: Handle<StandardMaterial>,
    /// 幽灵（蓝图不匹配，红）
    pub ghost_bad_material: Handle<StandardMaterial>,
    /// 幽灵蓝图材质：def.id → 半透明
    pub blueprint_materials: HashMap<String, Handle<StandardMaterial>>,
    /// 幽灵蓝图单位格网格（0.92 立方，略小于格子便于辨认）
    pub blueprint_unit_mesh: Handle<Mesh>,
}

/// 已放置积木：撤销历史 + 重做栈 + 占用集合 + 列顶高度（ADR-005 O(1) 查重）。
///
/// Command 模式（PRD §4.4）：放置即一条命令记录；撤销（Backspace / Ctrl+Z）
/// 移入重做栈，重做（Ctrl+Y）重新执行（重建实体）。历史上限 MAX_HISTORY，
/// 超限时丢弃最旧记录（同时销毁其实体）。
#[derive(Resource, Default)]
pub struct PlacedBlocks {
    /// 撤销历史（最近 MAX_HISTORY 条放置记录）
    pub records: Vec<PlacedRecord>,
    /// 重做栈（记录实体已销毁，重做时重建）
    pub redo: Vec<PlacedRecord>,
    pub occupied: HashSet<IVec3>,
    /// (x, z) → 该列当前最高已占用行 + 1（即下一层落位高度）
    pub col_top: HashMap<(i32, i32), i32>,
    /// 世界变更版本号：放置/撤销/重做/读档/清空时递增，
    /// 供集合扫描等 O(n) 系统做增量门控（B-20 性能优化）
    pub revision: u64,
}

/// 一条放置记录（撤销栈元素）。
pub struct PlacedRecord {
    pub entity: Entity,
    pub def_id: String,
    /// 锚点格（footprint 左上角，含 y）
    pub anchor: IVec3,
    /// 旋转（90°×rot）
    pub rot: u8,
    /// 占用格（3D）
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
    let mut blueprint_materials = HashMap::new();
    for def in &library.defs {
        let (w, h, d) = (def.size[0] as f32, def.size[1] as f32, def.size[2] as f32);
        // 程序化建筑网格（B-07 形制升级）：按积木 ID 选用对应几何
        let mesh = meshes.add(match def.id.as_str() {
            "hongzhu" | "hongzhu4" | "fangzhu" => {
                crate::building::meshes::column(0.5, h * GRID, 10)
            }
            "dengzhu" => crate::building::meshes::column(0.22, h * GRID, 8),
            "liuliwa" | "chuiwa" => crate::building::meshes::sloped_tile(w * GRID, d * GRID, 0.26),
            "feiyan" | "qiaoshou" => crate::building::meshes::sloped_tile(w * GRID, d * GRID, 0.45),
            "louban" | "louban5" => crate::building::meshes::eave_slab(w * GRID, d * GRID, 0.5),
            "taiji" | "datiji" => crate::building::meshes::eave_slab(w * GRID, d * GRID, 0.5),
            "baoding" => crate::building::meshes::spire(0.9),
            "jizhuanding" => crate::building::meshes::conical_roof(w * GRID, d * GRID, h * GRID, 8),
            _ => Mesh::from(Cuboid::new(w * GRID, h * GRID, d * GRID)),
        });
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(def.color[0], def.color[1], def.color[2], def.color[3]),
            perceptual_roughness: 0.55,
            ..default()
        });
        per_def.insert(def.id.clone(), (mesh, mat));

        // 幽灵蓝图：半透明（保留积木本色）
        let ghost_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(def.color[0], def.color[1], def.color[2], 0.35),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        blueprint_materials.insert(def.id.clone(), ghost_mat);
    }
    commands.insert_resource(BlockRenderAssets {
        per_def,
        ghost_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.85, 0.45, 0.35),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        ghost_bad_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.9, 0.25, 0.2, 0.4),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        blueprint_materials,
        blueprint_unit_mesh: meshes.add(Cuboid::new(0.92, 0.92, 0.92)),
    });
}

// ---------- 几何与匹配辅助 ----------

/// 鼠标位置 → y=0 地面上的列 (x, z)。
/// 排除离屏截图相机（B-21 引入的第二个 Camera3d），否则 single() 会因多匹配失效。
fn cursor_column(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<
        (&Camera, &GlobalTransform),
        (With<Camera3d>, Without<crate::screenshot::CaptureCamera>),
    >,
) -> Option<(i32, i32)> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_gt) = cameras.single().ok()?;
    let ray = camera.viewport_to_world(cam_gt, cursor).ok()?;
    let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    let hit = ray.get_point(t);
    Some(((hit.x / GRID).round() as i32, (hit.z / GRID).round() as i32))
}

/// 旋转后的平面 footprint 尺寸 (w, d)：90°/270° 时 w/d 互换。
fn rotated_footprint(def: &BlockDef, rot: u8) -> (u32, u32) {
    if rot % 2 == 1 {
        (def.size[2], def.size[0])
    } else {
        (def.size[0], def.size[2])
    }
}

/// 以光标格为中心对齐的 footprint 锚点（左上角格，仅 x/z）。
fn anchor_xz(center: (i32, i32), def: &BlockDef, rot: u8) -> (i32, i32) {
    let (w, d) = rotated_footprint(def, rot);
    (center.0 - (w as i32 - 1) / 2, center.1 - (d as i32 - 1) / 2)
}

/// footprint 覆盖的列集合。
pub(crate) fn footprint_columns(
    def: &BlockDef,
    anchor_x: i32,
    anchor_z: i32,
    rot: u8,
) -> Vec<(i32, i32)> {
    let (w, d) = rotated_footprint(def, rot);
    let (w, d) = (w as i32, d as i32);
    let mut cols = Vec::with_capacity((w * d) as usize);
    for dx in 0..w {
        for dz in 0..d {
            cols.push((anchor_x + dx, anchor_z + dz));
        }
    }
    cols
}

/// 列顶最高值 → 自由模式落位高度。
fn base_y_for(col_top: &HashMap<(i32, i32), i32>, cols: &[(i32, i32)]) -> i32 {
    cols.iter()
        .map(|c| col_top.get(c).copied().unwrap_or(0))
        .max()
        .unwrap_or(0)
}

/// footprint 占用格（3D，y 为底行）。
pub(crate) fn footprint_cells(anchor: IVec3, def: &BlockDef, rot: u8) -> Vec<IVec3> {
    let (w, d) = rotated_footprint(def, rot);
    let (w, h, d) = (w as i32, def.size[1] as i32, d as i32);
    let mut cells = Vec::with_capacity((w * h * d) as usize);
    for dy in 0..h {
        for dz in 0..d {
            for dx in 0..w {
                cells.push(IVec3::new(anchor.x + dx, anchor.y + dy, anchor.z + dz));
            }
        }
    }
    cells
}

/// 积木渲染中心（底面贴 anchor.y；按旋转后的 footprint 计算）。
pub(crate) fn block_center(anchor: IVec3, def: &BlockDef, rot: u8) -> Vec3 {
    let (w, d) = rotated_footprint(def, rot);
    let (w, h, d) = (w as f32, def.size[1] as f32, d as f32);
    Vec3::new(
        (anchor.x as f32 + (w - 1.0) * 0.5) * GRID,
        (anchor.y as f32 + h * 0.5) * GRID,
        (anchor.z as f32 + (d - 1.0) * 0.5) * GRID,
    )
}

/// 实体旋转（90°×rot，绕 Y 轴）。
pub(crate) fn rotation_quat(rot: u8) -> Quat {
    Quat::from_rotation_y(rot as f32 * std::f32::consts::FRAC_PI_2)
}

/// 计算放置锚点：
/// - 自由模式：按列顶堆叠；
/// - 蓝图模式：扫描 y 寻找使 footprint 完全匹配蓝图的落位（严格吸附，
///   旋转方向由玩家 R 键控制，与蓝图朝向一致时幽灵变绿）。
fn placement_anchor(
    col: (i32, i32),
    def: &BlockDef,
    rot: u8,
    stack: &PlacedBlocks,
    blueprint: &Blueprint,
) -> Option<IVec3> {
    if blueprint.active {
        // 蓝图模式：宽容定位——光标落在 footprint 内即可。
        // 从最接近中心的光标偏移开始搜索合法锚点（优先居中）。
        // （原实现只试"光标为中心"的单一锚点，5×5 台基等大块只有 1 个
        //   有效光标位，教程几乎无法完成——用户反馈后修复）
        let (w, d) = rotated_footprint(def, rot);
        let (w, d) = (w as i32, d as i32);
        let mut offsets: Vec<(i32, i32)> = Vec::with_capacity((w * d) as usize);
        for dx in 0..w {
            for dz in 0..d {
                offsets.push((dx, dz));
            }
        }
        // 按与中心偏移的距离排序：|dx - (w-1)/2| + |dz - (d-1)/2|
        let (cw, cd) = ((w - 1) / 2, (d - 1) / 2);
        offsets.sort_by_key(|(dx, dz)| (dx - cw).abs() + (dz - cd).abs());
        for (dx, dz) in offsets {
            let ax = col.0 - dx;
            let az = col.1 - dz;
            for y in 0..=MAX_BLUEPRINT_Y {
                let anchor = IVec3::new(ax, y, az);
                let cells = footprint_cells(anchor, def, rot);
                if footprint_matches(&blueprint.expected, &cells, &def.id) {
                    return Some(anchor);
                }
            }
        }
        None
    } else {
        let (ax, az) = anchor_xz(col, def, rot);
        let cols = footprint_columns(def, ax, az, rot);
        let y = base_y_for(&stack.col_top, &cols);
        Some(IVec3::new(ax, y, az))
    }
}

/// 由放置记录重建「列顶高度」映射（撤销后使用）。
pub(crate) fn rebuild_col_top_mut(stack: &mut PlacedBlocks) {
    let mut col_top: HashMap<(i32, i32), i32> = HashMap::new();
    for record in &stack.records {
        for cell in &record.cells {
            let entry = col_top.entry((cell.x, cell.z)).or_insert(0);
            *entry = (*entry).max(cell.y + 1);
        }
    }
    stack.col_top = col_top;
}

/// 生成积木实体（放置/重做/读档/复原共用）。
pub(crate) fn spawn_block_entity(
    commands: &mut Commands,
    library: &BlockLibrary,
    render: &BlockRenderAssets,
    def_id: &str,
    anchor: IVec3,
    rot: u8,
) -> Entity {
    let def = &library.defs[library.by_id[def_id]];
    let (mesh, mat) = render.per_def.get(def_id).expect("积木资产应已预生成");
    commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(block_center(anchor, def, rot))
                .with_rotation(rotation_quat(rot)),
            PlacedBlock,
            Name::new(format!("Block:{def_id}")),
        ))
        .id()
}

/// 覆盖光标列的最高积木记录索引（拆除用，纯查询）。
pub(crate) fn top_block_index_at(records: &[PlacedRecord], col: (i32, i32)) -> Option<usize> {
    records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.cells.iter().any(|c| c.x == col.0 && c.z == col.1))
        .max_by_key(|(_, r)| r.anchor.y)
        .map(|(i, _)| i)
}

// ---------- 系统 ----------

/// M 键切换蓝图模式（幽灵实体由 reconcile_blueprint_ghosts 对账生成/销毁）。
fn toggle_blueprint(keys: Res<ButtonInput<KeyCode>>, mut blueprint: ResMut<Blueprint>) {
    if keys.just_pressed(KeyCode::KeyM) {
        blueprint.active = !blueprint.active;
    }
}

/// 生成幽灵蓝图实体（按积木本色半透明）。
pub(crate) fn spawn_blueprint_ghosts(
    commands: &mut Commands,
    blueprint: &Blueprint,
    render: &BlockRenderAssets,
) {
    for cell in &blueprint.cell_list {
        let id = &blueprint.expected[cell];
        let mat = render
            .blueprint_materials
            .get(id)
            .cloned()
            .unwrap_or_else(|| render.ghost_material.clone());
        commands.spawn((
            Mesh3d(render.blueprint_unit_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(Vec3::new(
                cell.x as f32 + 0.5,
                cell.y as f32 + 0.5,
                cell.z as f32 + 0.5,
            )),
            BlueprintGhost,
            Name::new(format!("BlueprintGhost:{}", id)),
        ));
    }
}

/// 幽灵蓝图对账：蓝图开启且无幽灵 → 生成；关闭且有幽灵 → 销毁。
/// （M 键切换与教程强制开启共用此路径，保证实体与状态一致）
fn reconcile_blueprint_ghosts(
    mut commands: Commands,
    blueprint: Res<Blueprint>,
    existing: Query<Entity, With<BlueprintGhost>>,
    render: Res<BlockRenderAssets>,
) {
    let count = existing.iter().count();
    if blueprint.active && count == 0 {
        spawn_blueprint_ghosts(&mut commands, &blueprint, &render);
    } else if !blueprint.active && count > 0 {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
    }
}

/// 积木选择：数字键 1-9 直接选择；Q/E 循环切换；R 键旋转（90°步进）。
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
    if keys.just_pressed(KeyCode::KeyR) {
        library.rotation = (library.rotation + 1) % 4;
    }
}

/// 幽灵预览：跟随当前积木在光标列的落位；蓝图模式下红/绿反馈；
/// 挑战模式配额耗尽时同样显示红色。
/// 无障碍（B-25）：无效态除红色外叠加脉冲缩放——不依赖纯颜色的可辨反馈。
fn update_ghost_preview(
    mut commands: Commands,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<
        (&Camera, &GlobalTransform),
        (With<Camera3d>, Without<crate::screenshot::CaptureCamera>),
    >,
    mut ghost: Query<
        (
            Entity,
            &mut Transform,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<GhostBlock>,
    >,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<Blueprint>,
    challenge: Res<Challenge>,
    remove: Res<RemoveMode>,
) {
    let def = library.current_def();
    let (mesh_handle, _) = &render.per_def[&def.id];
    // 挑战限旋转：强制 0°
    let rot = if challenge.def.rotation_locked && challenge.is_active() {
        0
    } else {
        library.rotation
    };
    let col = cursor_column(&windows, &cameras);
    let anchor = if remove.active {
        None
    } else {
        col.and_then(|c| placement_anchor(c, def, rot, &stack, &blueprint))
    };
    let target = anchor.map(|a| block_center(a, def, rot));
    let ok = anchor.is_some() && challenge.can_place(&def.id);
    let ghost_mat = if ok {
        render.ghost_material.clone()
    } else {
        render.ghost_bad_material.clone()
    };
    let ghost_rot = rotation_quat(rot);
    // 无效态脉冲（无障碍：非纯颜色反馈，约 2.5 Hz 呼吸）
    let pulse = if ok {
        Vec3::ONE
    } else {
        let s = 1.0 + 0.18 * (time.elapsed_secs() * 16.0).sin().abs();
        Vec3::splat(s)
    };

    let mut existing = ghost.single_mut().ok();
    match (existing.take(), target) {
        (None, Some(pos)) => {
            commands.spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(ghost_mat),
                Transform::from_translation(pos)
                    .with_rotation(ghost_rot)
                    .with_scale(pulse),
                GhostBlock,
                Name::new("GhostBlock"),
            ));
        }
        (Some((_e, mut transform, mut mesh, mut mat)), Some(pos)) => {
            transform.translation = pos;
            transform.rotation = ghost_rot;
            transform.scale = pulse;
            if mesh.0 != *mesh_handle {
                mesh.0 = mesh_handle.clone();
            }
            mat.0 = ghost_mat;
        }
        (Some((_e, mut transform, _mesh, _mat)), None) => {
            transform.translation = Vec3::new(0.0, -1000.0, 0.0);
        }
        (None, None) => {}
    }
}

/// 放置与撤销/重做（Command 模式，PRD §4.4）：
/// - 撤销：Backspace 或 Ctrl/Cmd+Z —— 移除最后一次放置，移入重做栈
/// - 重做：Ctrl/Cmd+Y —— 重建上次撤销的积木
/// - 左键点击（非拖拽）：在幽灵所在位置放置当前积木（占用/蓝图匹配校验）
fn handle_place_and_undo(
    mut commands: Commands,
    mut click: ResMut<ClickState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<
        (&Camera, &GlobalTransform),
        (With<Camera3d>, Without<crate::screenshot::CaptureCamera>),
    >,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    audio: Res<AudioAssets>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stack: ResMut<PlacedBlocks>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    remove: Res<RemoveMode>,
) {
    let modifier = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    let rot_override = challenge.def.rotation_locked && challenge.is_active();

    // 撤销
    if keys.just_pressed(KeyCode::Backspace) || (modifier && keys.just_pressed(KeyCode::KeyZ)) {
        if let Some(record) = stack.records.pop() {
            for c in &record.cells {
                stack.occupied.remove(c);
            }
            rebuild_col_top_mut(&mut stack);
            commands.entity(record.entity).despawn();
            // 挑战配额退返
            if challenge.is_active() {
                challenge.refund(&record.def_id);
            }
            stack.redo.push(record);
            stack.revision += 1;
            if blueprint.active {
                refresh_completion(&stack, &library, &mut blueprint);
            }
        }
        return;
    }

    // 重做：重建实体并重新占用（挑战中需配额足够）
    if modifier && keys.just_pressed(KeyCode::KeyY) {
        let can_redo = stack
            .redo
            .last()
            .is_none_or(|r| !challenge.is_active() || challenge.can_place(&r.def_id));
        if can_redo {
            if let Some(record) = stack.redo.pop() {
                let def = &library.defs[library.by_id[&record.def_id]];
                let entity = spawn_block_entity(
                    &mut commands,
                    &library,
                    &render,
                    &record.def_id,
                    record.anchor,
                    record.rot,
                );
                for c in &record.cells {
                    stack.occupied.insert(*c);
                }
                let h = def.size[1] as i32;
                for (x, z) in footprint_columns(def, record.anchor.x, record.anchor.z, record.rot) {
                    let top = stack.col_top.entry((x, z)).or_insert(0);
                    *top = (*top).max(record.anchor.y + h);
                }
                if challenge.is_active() {
                    challenge.consume(&record.def_id);
                }
                stack.records.push(PlacedRecord {
                    entity,
                    def_id: record.def_id.clone(),
                    anchor: record.anchor,
                    rot: record.rot,
                    cells: record.cells.clone(),
                });
                stack.revision += 1;
                if blueprint.active {
                    refresh_completion(&stack, &library, &mut blueprint);
                }
            }
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
            if let Some(col) = cursor_column(&windows, &cameras) {
                // 拆除模式：移除光标列最顶部的积木
                if remove.active {
                    if let Some(idx) = top_block_index_at(&stack.records, col) {
                        let record = stack.records.remove(idx);
                        for c in &record.cells {
                            stack.occupied.remove(c);
                        }
                        rebuild_col_top_mut(&mut stack);
                        commands.entity(record.entity).despawn();
                        stack.revision += 1;
                        if blueprint.active {
                            refresh_completion(&stack, &library, &mut blueprint);
                        }
                        click.placed_this_press = true;
                    }
                    click.pressed_at = None;
                    return;
                }

                let def = library.current_def();
                let rot = if rot_override { 0 } else { library.rotation };
                if let Some(anchor) = placement_anchor(col, def, rot, &stack, &blueprint) {
                    let cells = footprint_cells(anchor, def, rot);
                    let free = cells.iter().all(|c| !stack.occupied.contains(c));
                    let quota_ok = challenge.can_place(&def.id);
                    if free && quota_ok {
                        let entity = spawn_block_entity(
                            &mut commands,
                            &library,
                            &render,
                            &def.id,
                            anchor,
                            rot,
                        );
                        for c in &cells {
                            stack.occupied.insert(*c);
                        }
                        let (ax, az) = (anchor.x, anchor.z);
                        for (x, z) in footprint_columns(def, ax, az, rot) {
                            let top = stack.col_top.entry((x, z)).or_insert(0);
                            *top = (*top).max(anchor.y + def.size[1] as i32);
                        }
                        if challenge.is_active() {
                            challenge.consume(&def.id);
                        }
                        stack.records.push(PlacedRecord {
                            entity,
                            def_id: def.id.clone(),
                            anchor,
                            rot,
                            cells,
                        });
                        click.placed_this_press = true;

                        // 放置音效（B-19）：按积木分类
                        play_placement_sound(&mut commands, &audio, sound_kind_for(&def.category));

                        // 新放置清空重做栈（标准撤销/重做语义）
                        stack.redo.clear();

                        // 历史上限：超限丢弃最旧记录（销毁其实体）
                        let mut dropped = false;
                        while stack.records.len() > MAX_HISTORY {
                            let oldest = stack.records.remove(0);
                            for c in &oldest.cells {
                                stack.occupied.remove(c);
                            }
                            commands.entity(oldest.entity).despawn();
                            dropped = true;
                        }
                        if dropped {
                            // 丢弃最旧后列顶可能失效，重建（O(n)，低频）
                            rebuild_col_top_mut(&mut stack);
                        }
                        stack.revision += 1;

                        if blueprint.active {
                            refresh_completion(&stack, &library, &mut blueprint);
                        }
                    }
                }
            }
        }
        click.pressed_at = None;
    }
}

/// 蓝图模式下实时刷新完成度；≥95% 触发完成事件。
pub(crate) fn refresh_completion(
    stack: &PlacedBlocks,
    library: &BlockLibrary,
    blueprint: &mut Blueprint,
) {
    let placed: HashMap<IVec3, String> = stack
        .records
        .iter()
        .flat_map(|r| r.cells.iter().map(move |c| (*c, r.def_id.clone())))
        .collect();
    blueprint.completion = compute_completion(&blueprint.expected, &placed, library);
    if blueprint.completion >= 0.95 && !blueprint.completed {
        blueprint.completed = true;
        info!(
            "🏛️ 黄鹤楼复原完成！完成度 {:.0}%",
            blueprint.completion * 100.0
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::block_defs::load_block_library;

    #[test]
    fn rotation_swaps_footprint() {
        let lib = load_block_library();
        let def = &lib.defs[lib.by_id["liangfang"]]; // 4×1×1
        let rot0 = footprint_cells(IVec3::new(0, 0, 0), def, 0);
        let rot1 = footprint_cells(IVec3::new(0, 0, 0), def, 1);
        assert_eq!(rot0.len(), 4);
        assert_eq!(rot1.len(), 4);
        // rot0 沿 x 展开 4 格；rot1 沿 z 展开 4 格
        assert!(rot0.contains(&IVec3::new(3, 0, 0)));
        assert!(!rot0.contains(&IVec3::new(0, 0, 3)));
        assert!(rot1.contains(&IVec3::new(0, 0, 3)));
        assert!(!rot1.contains(&IVec3::new(3, 0, 0)));
    }

    #[test]
    fn rotation_180_same_as_0() {
        let lib = load_block_library();
        let def = &lib.defs[lib.by_id["liangfang"]];
        let rot0 = footprint_cells(IVec3::new(0, 0, 0), def, 0);
        let rot2 = footprint_cells(IVec3::new(0, 0, 0), def, 2);
        let key = |c: &IVec3| (c.x, c.y, c.z);
        let mut a: Vec<_> = rot0.iter().map(key).collect();
        let mut b: Vec<_> = rot2.iter().map(key).collect();
        a.sort();
        b.sort();
        assert_eq!(a, b, "180° 旋转 footprint 应与 0° 一致");
    }

    #[test]
    fn blueprint_tolerant_anchor_covers_full_footprint() {
        let lib = load_block_library();
        let (mut bp, _) = crate::building::blueprint::load_blueprint_library();
        bp.active = true; // 蓝图分支（load 返回的 bp.active 为 false）
        let stack = PlacedBlocks::default();
        let datiji = &lib.defs[lib.by_id["datiji"]]; // 7×2×7
                                                     // 台基区域内任意光标格都应宽容定位到唯一合法锚点 (-3,0,-3)
        for (cx, cz) in [(-3, -3), (1, 1), (-1, 2), (0, 0), (2, -1), (3, 2)] {
            let anchor = placement_anchor((cx, cz), datiji, 0, &stack, &bp);
            assert_eq!(
                anchor,
                Some(IVec3::new(-3, 0, -3)),
                "光标 ({cx},{cz}) 应宽容定位到大台基锚点"
            );
        }
        // 区域外（如 (5,5)）仍应失败
        assert_eq!(placement_anchor((5, 5), datiji, 0, &stack, &bp), None);
    }

    #[test]
    fn blueprint_tolerant_anchor_beam_any_cell() {
        let lib = load_block_library();
        let (mut bp, _) = crate::building::blueprint::load_blueprint_library();
        bp.active = true;
        let stack = PlacedBlocks::default();
        let liangfang = &lib.defs[lib.by_id["liangfang"]]; // 4×1×1
                                                           // 二层梁枋（y=11，x -2..1 沿 z=-2 与 z=1）：光标在梁身任意格都可放置
        for (cx, cz) in [(-2, -2), (-1, -2), (0, -2), (1, -2), (0, 1), (1, 1)] {
            let anchor = placement_anchor((cx, cz), liangfang, 0, &stack, &bp);
            assert!(anchor.is_some(), "光标 ({cx},{cz}) 应可放置梁枋");
            let a = anchor.unwrap();
            assert_eq!(a.y, 11, "梁枋应在 y=11（二层重檐）");
        }
        // 不在梁枋行的位置应失败
        assert_eq!(placement_anchor((0, 3), liangfang, 0, &stack, &bp), None);
    }
}

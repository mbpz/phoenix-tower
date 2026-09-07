//! 积木放置系统（B-08 三维版 + 蓝图模式）。
//!
//! 主链路：`射线检测 → 候选网格位置 → 合法性校验（占用/蓝图匹配）→ 生成 Entity`。
//!
//! - 三维堆叠：光标列上按「列顶高度」落位，可向上建造多层；
//! - 蓝图模式（M 键）：幽灵蓝图 + 严格吸附——仅当放置块 footprint 与蓝图
//!   期望完全一致才允许放置（幽灵红/绿反馈），完成度实时计算（ADR-005）；
//! - 完整建筑与 20 步撤销历史独立维护（见 world 模块）。

use crate::ui::input::{keyboard_allowed, shortcuts_allowed, InputOwnership};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashMap;

use super::block_defs::{load_block_library, BlockDef, BlockLibrary};
use super::blueprint::{
    compute_completion, footprint_matches, load_blueprint_library, Blueprint, BlueprintGhost,
};
use super::challenge::Challenge;
use super::decorations::attach_block_decorations;
pub use super::decorations::{LanternLight, PlaqueText};
use crate::audio::{play_placement_sound, sound_kind_for, AudioAssets};
use crate::stability::RemoveMode;

pub struct PlacementPlugin;

#[derive(Message)]
pub(crate) struct BlueprintPlacementUndone;

impl Plugin for PlacementPlugin {
    fn build(&self, app: &mut App) {
        let (blueprint, blueprint_library) = load_blueprint_library();
        app.add_message::<BlueprintPlacementUndone>()
            .insert_resource(load_block_library())
            .insert_resource(blueprint)
            .insert_resource(blueprint_library)
            .insert_resource(PlacedBlocks::default())
            .insert_resource(BlueprintAlpha::default())
            .add_systems(Startup, setup_block_assets)
            .add_systems(
                Update,
                (
                    toggle_blueprint,
                    reconcile_blueprint_ghosts,
                    attach_block_decorations,
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

// 兼容既有调用路径；数据变更集中在 world 模块。
pub use super::world::{PlacedBlocks, PlacedRecord};

/// 幽灵预览标记
#[derive(Component)]
pub struct GhostBlock;

/// 已放置积木标记
#[derive(Component)]
pub struct PlacedBlock;

/// 积木 ID 组件（挂在实际实体上，供光源对账等系统按类型查询）
#[derive(Component, Clone)]
pub struct BlockId(pub String);

fn setup_block_assets(
    asset_server: Res<AssetServer>,
    mode: Res<crate::riverside::RiversideMode>,
    mut commands: Commands,
    library: Res<BlockLibrary>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<bevy::image::Image>>,
) {
    // 琉璃瓦勾缝纹理（B-07 形制：金色瓦面 + 勾缝 + 噪声）
    let tile = images.add(crate::building::meshes::tile_texture(20260828));
    let mut per_def = HashMap::new();
    let mut blueprint_materials = HashMap::new();
    for def in &library.defs {
        let (w, h, d) = (def.size[0] as f32, def.size[1] as f32, def.size[2] as f32);
        // 程序化建筑网格（B-07 形制升级）：按积木 ID 选用对应几何
        let model = crate::riverside::model_path(&def.id)
            .filter(|_| mode.0)
            .filter(|path| {
                let exists = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join(path)
                    .is_file();
                if !exists {
                    warn!("Missing Blender model {path}; using procedural mesh");
                }
                exists
            });
        let mesh = if let Some(path) = model {
            asset_server.load(
                bevy::gltf::GltfAssetLabel::Primitive {
                    mesh: 0,
                    primitive: 0,
                }
                .from_asset(path),
            )
        } else {
            meshes.add(match def.id.as_str() {
                "hongzhu" | "hongzhu4" | "fangzhu" => {
                    crate::building::meshes::column(0.5, h * GRID, 10)
                        .translated_by(Vec3::Y * (-h * GRID * 0.5))
                }
                "dengzhu" => crate::building::meshes::column(0.22, h * GRID, 8),
                "denglong" => crate::building::meshes::column(0.42, 0.7, 8),
                "biane" => crate::building::meshes::plaque(),
                "liuliwa" | "chuiwa" => {
                    crate::building::meshes::sloped_tile(w * GRID, d * GRID, 0.26)
                }
                "feiyan" | "qiaoshou" => {
                    crate::building::meshes::sloped_tile(w * GRID, d * GRID, 0.45)
                }
                "dougong" => crate::building::meshes::dougong_bracket(),
                "louban" | "louban5" => {
                    crate::building::meshes::eave_slab(w * GRID, d * GRID, 0.5, 0.9)
                }
                "taiji" | "datiji" => Mesh::from(Cuboid::new(w * GRID, h * GRID, d * GRID)),
                "baoding" => crate::building::meshes::spire(0.9),
                "jizhuanding" | "jiangting_roof" => {
                    crate::building::meshes::conical_roof(w * GRID, d * GRID, h * GRID, 8)
                }
                _ => Mesh::from(Cuboid::new(w * GRID, h * GRID, d * GRID)),
            })
        };
        let is_roof = matches!(
            def.id.as_str(),
            "liuliwa"
                | "chuiwa"
                | "feiyan"
                | "qiaoshou"
                | "jizhuanding"
                | "baoding"
                | "jiangting_roof"
        );
        // Playable pieces and placement guides must stay legible at any orbit
        // distance. Keep distance fog on the landscape, not interactive geometry.
        let mat = materials.add(StandardMaterial {
            fog_enabled: false,
            base_color: if model.is_some() {
                Color::WHITE
            } else {
                Color::srgba(def.color[0], def.color[1], def.color[2], def.color[3])
            },
            base_color_texture: if is_roof && model.is_none() {
                Some(tile.clone())
            } else {
                None
            },
            perceptual_roughness: if is_roof { 0.35 } else { 0.55 },
            ..default()
        });
        per_def.insert(def.id.clone(), (mesh, mat));

        // 幽灵蓝图：半透明（保留积木本色）
        let ghost_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(def.color[0], def.color[1], def.color[2], 0.35),
            unlit: true,
            fog_enabled: false,
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
            fog_enabled: false,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        ghost_bad_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.9, 0.25, 0.2, 0.4),
            unlit: true,
            fog_enabled: false,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        blueprint_materials,
        blueprint_unit_mesh: meshes.add(Cuboid::new(0.92, 0.92, 0.92)),
    });
}

// ---------- 几何与匹配辅助 ----------

type PlacementCameras<'w, 's> = Query<
    'w,
    's,
    (&'static Camera, &'static GlobalTransform),
    (With<Camera3d>, Without<crate::screenshot::CaptureCamera>),
>;

/// 鼠标位置 → y=0 地面上的列 (x, z)。
/// 排除离屏截图相机（B-21 引入的第二个 Camera3d），否则 single() 会因多匹配失效。
fn cursor_column(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &PlacementCameras<'_, '_>,
) -> Option<(i32, i32)> {
    let ray = cursor_ray(windows, cameras)?;
    let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    let hit = ray.get_point(t);
    Some(((hit.x / GRID).round() as i32, (hit.z / GRID).round() as i32))
}

fn cursor_ray(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &PlacementCameras<'_, '_>,
) -> Option<Ray3d> {
    let cursor = windows.single().ok()?.cursor_position()?;
    let (camera, transform) = cameras.single().ok()?;
    camera.viewport_to_world(transform, cursor).ok()
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

/// Column selection remains useful for free stacking and top-down tests.
fn placement_anchor(
    col: (i32, i32),
    def: &BlockDef,
    rot: u8,
    stack: &PlacedBlocks,
    blueprint: &Blueprint,
) -> Option<IVec3> {
    if blueprint.active {
        let (w, d) = rotated_footprint(def, rot);
        blueprint
            .cell_list
            .iter()
            .copied()
            .filter(|anchor| {
                anchor.x <= col.0
                    && col.0 < anchor.x + w as i32
                    && anchor.z <= col.1
                    && col.1 < anchor.z + d as i32
                    && blueprint.expected.get(anchor) == Some(&def.id)
                    && {
                        let cells = footprint_cells(*anchor, def, rot);
                        cells.iter().all(|c| !stack.occupied.contains(c))
                            && footprint_matches(&blueprint.expected, &cells, &def.id)
                    }
            })
            .min_by_key(|a| (a.y, a.x, a.z))
    } else {
        let (ax, az) = anchor_xz(col, def, rot);
        let cols = footprint_columns(def, ax, az, rot);
        Some(IVec3::new(ax, base_y_for(&stack.col_top, &cols), az))
    }
}

/// Preview and commit use the same visible, unoccupied blueprint footprint.
/// Each anchor must itself be an expected cell: work is bounded by blueprint
/// size, not a speculative height scan. Test the ray before allocating cells.
fn ray_placement_anchor(
    ray: Ray3d,
    def: &BlockDef,
    rot: u8,
    stack: &PlacedBlocks,
    blueprint: &Blueprint,
) -> Option<IVec3> {
    if !blueprint.active {
        let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
        let hit = ray.get_point(t);
        return placement_anchor(
            (hit.x.round() as i32, hit.z.round() as i32),
            def,
            rot,
            stack,
            blueprint,
        );
    }
    use bevy::math::bounding::{Aabb3d, RayCast3d};
    let cast = RayCast3d::from_ray(ray, f32::MAX);
    // Occupied grid cells are conservative occluders: never place through a
    // completed front piece merely because the rear one matches the material.
    let occlusion = stack
        .occupied
        .iter()
        .filter_map(|cell| {
            cast.aabb_intersection_at(&Aabb3d::new(
                cell.as_vec3() + Vec3::Y * 0.5,
                Vec3::splat(0.5),
            ))
        })
        .min_by(f32::total_cmp)
        .unwrap_or(f32::MAX);
    let (w, d) = rotated_footprint(def, rot);
    let half = Vec3::new(w as f32, def.size[1] as f32, d as f32) * 0.5;
    blueprint
        .cell_list
        .iter()
        .filter_map(|&anchor| {
            if blueprint.expected.get(&anchor) != Some(&def.id) || stack.occupied.contains(&anchor)
            {
                return None;
            }
            let distance =
                cast.aabb_intersection_at(&Aabb3d::new(block_center(anchor, def, rot), half))?;
            if distance > occlusion + 0.001 {
                return None;
            }
            let cells = footprint_cells(anchor, def, rot);
            (cells.iter().all(|c| !stack.occupied.contains(c))
                && footprint_matches(&blueprint.expected, &cells, &def.id))
            .then_some((distance, anchor))
        })
        .min_by(|(da, a), (db, b)| {
            da.total_cmp(db)
                .then_with(|| (a.y, a.x, a.z).cmp(&(b.y, b.x, b.z)))
        })
        .map(|(_, anchor)| anchor)
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
            BlockId(def_id.to_string()),
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
fn toggle_blueprint(
    ownership: Option<Res<InputOwnership>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut blueprint: ResMut<Blueprint>,
    stack: Res<PlacedBlocks>,
    library: Res<BlockLibrary>,
) {
    if shortcuts_allowed(&keys, ownership.as_deref()) && keys.just_pressed(KeyCode::KeyM) {
        blueprint.active = !blueprint.active;
        if blueprint.active {
            refresh_completion(&stack, &library, &mut blueprint);
        }
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
                cell.x as f32,
                cell.y as f32 + 0.5,
                cell.z as f32,
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
fn select_block(
    ownership: Option<Res<InputOwnership>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut library: ResMut<BlockLibrary>,
) {
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
        if shortcuts_allowed(&keys, ownership.as_deref())
            && keys.just_pressed(*key)
            && i < library.defs.len()
        {
            library.current = i;
            return;
        }
    }
    let len = library.defs.len();
    if shortcuts_allowed(&keys, ownership.as_deref()) && keys.just_pressed(KeyCode::KeyQ) {
        library.current = (library.current + len - 1) % len;
    }
    if shortcuts_allowed(&keys, ownership.as_deref()) && keys.just_pressed(KeyCode::KeyE) {
        library.current = (library.current + 1) % len;
    }
    if shortcuts_allowed(&keys, ownership.as_deref()) && keys.just_pressed(KeyCode::KeyR) {
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
    cameras: PlacementCameras<'_, '_>,
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
    let anchor = if remove.active {
        None
    } else {
        cursor_ray(&windows, &cameras)
            .and_then(|ray| ray_placement_anchor(ray, def, rot, &stack, &blueprint))
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
pub(crate) fn handle_place_and_undo(
    mut commands: Commands,
    ownership: Option<Res<InputOwnership>>,
    mut undone: MessageWriter<BlueprintPlacementUndone>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: PlacementCameras<'_, '_>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    audio: Res<AudioAssets>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stack: ResMut<PlacedBlocks>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    remove: Res<RemoveMode>,
) {
    let modifier = [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]
    .into_iter()
    .any(|key| crate::ui::input::modifier_active(&keys, key));
    let shift = [KeyCode::ShiftLeft, KeyCode::ShiftRight]
        .into_iter()
        .any(|key| crate::ui::input::modifier_active(&keys, key));
    let rot_override = challenge.def.rotation_locked && challenge.is_active();

    // 撤销
    if keyboard_allowed(ownership.as_deref())
        && (keys.just_pressed(KeyCode::Backspace)
            || (modifier && !shift && keys.just_pressed(KeyCode::KeyZ)))
    {
        if let Some(record) = stack.undo() {
            commands.entity(record.entity).despawn();
            // 挑战配额退返
            if challenge.is_active() {
                challenge.refund(&record.def_id);
            }
            if blueprint.active {
                undone.write(BlueprintPlacementUndone);
                refresh_completion(&stack, &library, &mut blueprint);
            }
        }
        return;
    }

    // 重做：重建实体并重新占用（挑战中需配额足够）
    if keyboard_allowed(ownership.as_deref())
        && modifier
        && (keys.just_pressed(KeyCode::KeyY) || (shift && keys.just_pressed(KeyCode::KeyZ)))
    {
        let can_redo = stack
            .redo
            .last()
            .is_none_or(|r| !challenge.is_active() || challenge.can_place(&r.def_id));
        if can_redo {
            if let Some(record) = stack.redo.last() {
                let entity = spawn_block_entity(
                    &mut commands,
                    &library,
                    &render,
                    &record.def_id,
                    record.anchor,
                    record.rot,
                );
                if challenge.is_active() {
                    challenge.consume(&record.def_id);
                }
                stack.redo(entity);
                if blueprint.active {
                    refresh_completion(&stack, &library, &mut blueprint);
                }
            }
        }
        return;
    }

    if !ownership.is_some_and(|input| input.world_click()) {
        return;
    }
    if let Some(ray) = cursor_ray(&windows, &cameras) {
        // 拆除模式：移除光标列最顶部的积木
        if remove.active {
            if let Some(idx) = cursor_column(&windows, &cameras)
                .and_then(|col| top_block_index_at(&stack.records, col))
            {
                let record = stack.remove(idx);
                commands.entity(record.entity).despawn();
                if blueprint.active {
                    refresh_completion(&stack, &library, &mut blueprint);
                }
            }
            return;
        }

        let def = library.current_def();
        let rot = if rot_override { 0 } else { library.rotation };
        if let Some(anchor) = ray_placement_anchor(ray, def, rot, &stack, &blueprint) {
            let cells = footprint_cells(anchor, def, rot);
            let free = cells.iter().all(|c| !stack.occupied.contains(c));
            let quota_ok = challenge.can_place(&def.id);
            if free && quota_ok {
                let entity =
                    spawn_block_entity(&mut commands, &library, &render, &def.id, anchor, rot);
                if challenge.is_active() {
                    challenge.consume(&def.id);
                }
                stack.place(PlacedRecord {
                    entity,
                    def_id: def.id.clone(),
                    anchor,
                    rot,
                    cells,
                });

                // 放置音效（B-19）：按积木分类
                play_placement_sound(&mut commands, &audio, sound_kind_for(&def.category));

                if blueprint.active {
                    refresh_completion(&stack, &library, &mut blueprint);
                }
            }
        }
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
    fn placed_front_column_occludes_unplaced_rear_column() {
        let lib = load_block_library();
        let mut bp = load_blueprint_library().0;
        bp.active = true;
        let def = &lib.defs[lib.by_id["hongzhu4"]];
        let mut stack = PlacedBlocks::default();
        let ray = Ray3d::new(Vec3::new(3.0, 4.0, 20.0), Dir3::NEG_Z);
        assert_eq!(
            ray_placement_anchor(ray, def, 0, &stack, &bp),
            Some(IVec3::new(3, 2, 3))
        );
        stack
            .occupied
            .extend(footprint_cells(IVec3::new(3, 2, 3), def, 0));
        assert_eq!(ray_placement_anchor(ray, def, 0, &stack, &bp), None);
    }

    #[test]
    fn ray_targets_visible_elevated_beam_instead_of_ground_behind_it() {
        let lib = load_block_library();
        let (mut bp, _) = crate::building::blueprint::load_blueprint_library();
        bp.active = true;
        let def = &lib.defs[lib.by_id["liangfang5"]];
        let anchor = IVec3::new(-2, 7, 2);
        let center = block_center(anchor, def, 0);
        let origin = center + Vec3::new(0.0, 5.0, 15.0);
        let ray = Ray3d::new(origin, Dir3::new(center - origin).unwrap());
        assert_eq!(
            ray_placement_anchor(ray, def, 0, &PlacedBlocks::default(), &bp),
            Some(anchor)
        );
    }

    #[test]
    fn occupied_platform_is_not_a_green_placement_target() {
        let lib = load_block_library();
        let (mut bp, _) = crate::building::blueprint::load_blueprint_library();
        bp.active = true;
        let def = &lib.defs[lib.by_id["datiji"]];
        let mut stack = PlacedBlocks::default();
        stack
            .occupied
            .extend(footprint_cells(IVec3::new(-3, 0, -3), def, 0));
        let ray = Ray3d::new(Vec3::new(0.0, 10.0, 0.0), Dir3::NEG_Y);
        assert_eq!(ray_placement_anchor(ray, def, 0, &stack, &bp), None);
    }

    #[test]
    fn tutorial_meshes_align_with_their_grid_height() {
        use bevy::mesh::VertexAttributeValues;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(crate::riverside::RiversideMode(false))
            .insert_resource(load_block_library())
            .add_systems(Startup, setup_block_assets);
        app.update();
        let render = app.world().resource::<BlockRenderAssets>();
        let meshes = app.world().resource::<Assets<Mesh>>();
        let library = app.world().resource::<BlockLibrary>();
        for id in ["datiji", "hongzhu4", "liangfang5"] {
            let def = &library.defs[library.by_id[id]];
            let mesh = meshes.get(&render.per_def[id].0).unwrap();
            let VertexAttributeValues::Float32x3(vertices) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
            else {
                panic!("position format");
            };
            let center = block_center(IVec3::new(0, 2, 0), def, 0);
            let min = vertices
                .iter()
                .map(|p| p[1] + center.y)
                .fold(f32::INFINITY, f32::min);
            let max = vertices
                .iter()
                .map(|p| p[1] + center.y)
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((min - 2.0).abs() < 0.001, "{id}: bottom={min}");
            assert!(
                (max - (2.0 + def.size[1] as f32)).abs() < 0.001,
                "{id}: top={max}"
            );
        }
    }

    #[test]
    fn interactive_materials_remain_readable_outside_landscape_fog_range() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(crate::riverside::RiversideMode(false))
            .insert_resource(load_block_library())
            .add_systems(Startup, setup_block_assets);
        app.update();
        let render = app.world().resource::<BlockRenderAssets>();
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        for (id, (_, handle)) in &render.per_def {
            assert!(
                !materials.get(handle).unwrap().fog_enabled,
                "placed {id} lost in fog"
            );
        }
        for handle in render
            .blueprint_materials
            .values()
            .chain([&render.ghost_material, &render.ghost_bad_material])
        {
            let material = materials.get(handle).unwrap();
            assert!(!material.fog_enabled, "placement guide lost in fog");
            assert!(material.unlit);
            assert_eq!(material.alpha_mode, AlphaMode::Blend);
        }
    }

    #[test]
    fn blueprint_unit_ghost_uses_same_grid_origin_as_real_blocks() {
        let library = load_block_library();
        let mut app = App::new();
        let (blueprint, _) = load_blueprint_library();
        let cell = blueprint.cell_list[0];
        app.insert_resource(blueprint)
            .insert_resource(BlockRenderAssets {
                per_def: Default::default(),
                ghost_material: Handle::default(),
                ghost_bad_material: Handle::default(),
                blueprint_materials: Default::default(),
                blueprint_unit_mesh: Handle::default(),
            })
            .add_systems(
                Startup,
                |mut commands: Commands, bp: Res<Blueprint>, render: Res<BlockRenderAssets>| {
                    spawn_blueprint_ghosts(&mut commands, &bp, &render);
                },
            );
        app.update();
        let mut unit = library.defs[0].clone();
        unit.size = [1, 1, 1];
        let expected = block_center(cell, &unit, 0);
        let mut query = app
            .world_mut()
            .query_filtered::<&Transform, With<BlueprintGhost>>();
        assert!(query.iter(app.world()).any(|tf| tf.translation == expected));
        assert!(query
            .iter(app.world())
            .all(|tf| tf.translation.x.fract() == 0.0 && tf.translation.z.fract() == 0.0));
    }

    #[test]
    fn enabling_blueprint_refreshes_completion_after_free_edits() {
        let mut app = App::new();
        let (mut blueprint, _) = load_blueprint_library();
        blueprint.completion = 1.0;
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyM);
        app.insert_resource(blueprint)
            .insert_resource(load_block_library())
            .insert_resource(PlacedBlocks::default())
            .insert_resource(keys)
            .add_systems(Update, toggle_blueprint);
        app.update();
        let blueprint = app.world().resource::<Blueprint>();
        assert!(blueprint.active);
        assert!(blueprint.completion < 0.95);
    }

    #[test]
    fn riverside_roof_snaps_back_without_overlapping_remaining_structure() {
        let library = load_block_library();
        let (_, themes) = load_blueprint_library();
        let def = &library.defs[library.by_id["jiangting_roof"]];
        let mut blueprint = crate::building::blueprint::blueprint_from_def(
            themes.defs.iter().find(|d| d.id == "riverside").unwrap(),
        );
        blueprint.active = true;
        let mut stack = PlacedBlocks::default();
        stack.occupied.extend(
            blueprint
                .expected
                .iter()
                .filter(|(_, id)| id.as_str() != "jiangting_roof")
                .map(|(cell, _)| *cell),
        );
        for rot in 0..4 {
            for column in [(0, 0), (-3, -3), (3, 3)] {
                let anchor = placement_anchor(column, def, rot, &stack, &blueprint).unwrap();
                assert_eq!(anchor, IVec3::new(-3, 7, -3));
                let cells = footprint_cells(anchor, def, rot);
                assert!(cells.iter().all(|cell| !stack.occupied.contains(cell)));
                assert!(footprint_matches(&blueprint.expected, &cells, &def.id));
            }
        }
    }

    #[test]
    fn column_heights_follow_remaining_cells_after_removal() {
        let mut stack = PlacedBlocks::default();
        stack.place(PlacedRecord {
            entity: Entity::PLACEHOLDER,
            def_id: "test".into(),
            anchor: IVec3::ZERO,
            rot: 0,
            cells: vec![IVec3::ZERO, IVec3::Y, IVec3::new(1, 0, 0)],
        });
        assert_eq!(stack.col_top.get(&(0, 0)), Some(&2));
        assert_eq!(stack.col_top.get(&(1, 0)), Some(&1));
        stack.clear();
        assert!(stack.col_top.is_empty());
    }

    fn history_input_app() -> App {
        let mut app = App::new();
        let library = load_block_library();
        let def = &library.defs[library.by_id["taiji"]];
        let entity = app.world_mut().spawn(PlacedBlock).id();
        let mut stack = PlacedBlocks::default();
        let cells = footprint_cells(IVec3::ZERO, def, 0);
        stack.place(PlacedRecord {
            entity,
            def_id: "taiji".into(),
            anchor: IVec3::ZERO,
            rot: 0,
            cells,
        });
        let mut blueprint = crate::building::blueprint::load_blueprint();
        blueprint.active = false;
        let mut challenge = crate::building::challenge::load_challenge();
        challenge.state = crate::building::challenge::ChallengeState::Active;
        challenge.quota_left.insert("taiji".into(), 0);
        app.insert_resource(stack)
            .insert_resource(library)
            .insert_resource(blueprint)
            .insert_resource(challenge)
            .insert_resource(BlockRenderAssets {
                per_def: [("taiji".into(), (Handle::default(), Handle::default()))].into(),
                ghost_material: Handle::default(),
                ghost_bad_material: Handle::default(),
                blueprint_materials: Default::default(),
                blueprint_unit_mesh: Handle::default(),
            })
            .insert_resource(AudioAssets {
                place_wood: Handle::default(),
                place_stone: Handle::default(),
                bell_chime: Handle::default(),
            })
            .add_message::<BlueprintPlacementUndone>()
            .init_resource::<RemoveMode>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<bevy::picking::hover::HoverMap>()
            .add_systems(Update, handle_place_and_undo);
        app
    }

    fn press_history_keys(app: &mut App, keys: &[KeyCode]) {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.reset_all();
        for key in keys {
            input.press(*key);
        }
        app.update();
    }

    #[test]
    fn focused_path_blocks_world_history_and_selection_hotkeys() {
        use crate::ui::input::InputOwnershipPlugin;
        use crate::ui::lunex::{LunexTab, LunexTabId, PathInput};

        let mut app = history_input_app();
        app.add_plugins(InputOwnershipPlugin)
            .insert_resource(PathInput {
                value: "/tmp/castle.ptw".into(),
                focused: true,
            })
            .insert_resource(LunexTab(LunexTabId::Saves))
            .add_systems(Update, select_block);
        let original = app.world().resource::<PlacedBlocks>().records[0].entity;
        let selected = app.world().resource::<BlockLibrary>().current;
        let rotation = app.world().resource::<BlockLibrary>().rotation;
        for keys in [
            vec![KeyCode::Backspace],
            vec![KeyCode::SuperLeft, KeyCode::KeyZ],
            vec![KeyCode::KeyE],
            vec![KeyCode::KeyR],
        ] {
            press_history_keys(&mut app, &keys);
            assert!(app.world().get_entity(original).is_ok());
            assert!(app.world().resource::<PlacedBlocks>().redo.is_empty());
            assert_eq!(app.world().resource::<BlockLibrary>().current, selected);
            assert_eq!(app.world().resource::<BlockLibrary>().rotation, rotation);
        }
        app.world_mut().resource_mut::<PathInput>().focused = false;
        press_history_keys(&mut app, &[KeyCode::Backspace]);
        assert!(app.world().get_entity(original).is_err());
        app.world_mut().resource_mut::<PathInput>().focused = true;
        for keys in [
            vec![KeyCode::SuperLeft, KeyCode::KeyY],
            vec![KeyCode::SuperLeft, KeyCode::ShiftLeft, KeyCode::KeyZ],
            vec![KeyCode::SuperRight, KeyCode::ShiftRight, KeyCode::KeyZ],
        ] {
            press_history_keys(&mut app, &keys);
            assert!(app.world().resource::<PlacedBlocks>().records.is_empty());
            assert_eq!(app.world().resource::<PlacedBlocks>().redo.len(), 1);
        }
    }

    #[test]
    fn fast_history_chords_survive_modifier_release_in_the_same_frame() {
        let mut app = history_input_app();
        for (chord, expected_count) in [
            (vec![KeyCode::SuperLeft, KeyCode::KeyZ], 0),
            (
                vec![KeyCode::SuperLeft, KeyCode::ShiftLeft, KeyCode::KeyZ],
                1,
            ),
        ] {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            for key in &chord {
                keys.press(*key);
            }
            for key in chord.iter().rev() {
                keys.release(*key);
            }
            app.update();
            assert_eq!(
                app.world().resource::<PlacedBlocks>().records.len(),
                expected_count
            );
        }
    }

    #[test]
    fn command_shift_z_redoes_without_undoing_and_restores_quota() {
        for command in [KeyCode::SuperLeft, KeyCode::SuperRight] {
            for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
                let mut app = history_input_app();
                let original = app.world().resource::<PlacedBlocks>().records[0].entity;
                let revision = app.world().resource::<PlacedBlocks>().revision;

                // An empty redo stack must not turn Cmd+Shift+Z into undo.
                press_history_keys(&mut app, &[command, shift, KeyCode::KeyZ]);
                assert!(app.world().get_entity(original).is_ok());
                assert_eq!(app.world().resource::<PlacedBlocks>().revision, revision);
                assert_eq!(app.world().resource::<Challenge>().quota_left["taiji"], 0);

                press_history_keys(&mut app, &[command, KeyCode::KeyZ]);
                assert!(app.world().get_entity(original).is_err());
                assert_eq!(app.world().resource::<Challenge>().quota_left["taiji"], 1);

                press_history_keys(&mut app, &[command, shift, KeyCode::KeyZ]);
                let stack = app.world().resource::<PlacedBlocks>();
                assert_eq!(stack.records.len(), 1);
                assert!(stack.redo.is_empty());
                assert_ne!(stack.records[0].entity, original);
                assert!(app
                    .world()
                    .get::<PlacedBlock>(stack.records[0].entity)
                    .is_some());
                assert_eq!(stack.occupied.len(), stack.records[0].cells.len());
                assert_eq!(app.world().resource::<Challenge>().quota_left["taiji"], 0);
            }
        }
    }

    #[test]
    fn keyboard_undo_redo_updates_entities_and_challenge_quota() {
        let mut app = history_input_app();
        let original = app.world().resource::<PlacedBlocks>().records[0].entity;
        press_history_keys(&mut app, &[KeyCode::Backspace]);
        assert!(app.world().get_entity(original).is_err());
        assert!(app.world().resource::<PlacedBlocks>().occupied.is_empty());
        assert_eq!(app.world().resource::<Challenge>().quota_left["taiji"], 1);
        press_history_keys(&mut app, &[KeyCode::ControlLeft, KeyCode::KeyY]);
        let stack = app.world().resource::<PlacedBlocks>();
        assert_eq!(stack.records.len(), 1);
        assert_ne!(stack.records[0].entity, original);
        assert!(app
            .world()
            .get::<PlacedBlock>(stack.records[0].entity)
            .is_some());
        assert_eq!(stack.occupied.len(), stack.records[0].cells.len());
        assert_eq!(app.world().resource::<Challenge>().quota_left["taiji"], 0);
    }

    #[test]
    fn only_successful_blueprint_undo_emits_activity() {
        let mut app = history_input_app();
        app.world_mut().resource_mut::<Blueprint>().active = true;
        let mut messages = bevy::ecs::message::MessageCursor::<BlueprintPlacementUndone>::default();
        press_history_keys(&mut app, &[KeyCode::Backspace]);
        assert_eq!(
            messages
                .read(app.world().resource::<Messages<BlueprintPlacementUndone>>())
                .count(),
            1
        );
        press_history_keys(&mut app, &[KeyCode::Backspace]);
        assert_eq!(
            messages
                .read(app.world().resource::<Messages<BlueprintPlacementUndone>>())
                .count(),
            0
        );
    }

    #[test]
    fn insufficient_quota_keeps_redo_and_revision_unchanged() {
        let mut app = history_input_app();
        press_history_keys(&mut app, &[KeyCode::Backspace]);
        let revision = app.world().resource::<PlacedBlocks>().revision;
        app.world_mut()
            .resource_mut::<Challenge>()
            .quota_left
            .insert("taiji".into(), 0);
        press_history_keys(&mut app, &[KeyCode::ControlLeft, KeyCode::KeyY]);
        let stack = app.world().resource::<PlacedBlocks>();
        assert!(stack.records.is_empty());
        assert_eq!(stack.redo.len(), 1);
        assert_eq!(stack.revision, revision);
        assert_eq!(app.world().resource::<Challenge>().quota_left["taiji"], 0);
    }

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

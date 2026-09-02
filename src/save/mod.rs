//! 本地存档（B-11）：`.ptw`（bincode 压缩）+ `.json` 导出。
//!
//! 格式与兼容策略见 ADR-006：
//! - `version` 只增不减；读档走迁移链；未知/未来版本拒绝加载
//! - 原子写入（tmp + rename）防中断损坏
//! - 存档仅含业务数据（id/cell/rot），不含实体句柄 —— 可跨版本回放
//!
//! 快捷键：F5 保存 .ptw / F6 导出 .json / F9 读取 .ptw

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::Blueprint;
use crate::building::challenge::{Challenge, ChallengeState};
use crate::building::placement::{
    footprint_cells, footprint_columns, refresh_completion, spawn_block_entity, BlockRenderAssets,
    PlacedBlock, PlacedBlocks, PlacedRecord,
};

/// 当前存档格式版本（ADR-006：只增不减）
pub const SAVE_VERSION: u32 = 1;
/// 主存档文件名
pub const SLOT_FILENAME: &str = "slot1.ptw";

/// 存档文件（.ptw，bincode 编码）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SaveFile {
    pub version: u32,
    /// "free" | "blueprint"（记录用；读取后不强制切换模式）
    pub game_mode: String,
    /// 挑战随机种子（ADR-002 物理确定性回放；Phase 2 使用）
    pub seed: u64,
    pub blocks: Vec<PlacedBlockRecord>,
    pub meta: SaveMeta,
}

/// 单个积木记录。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlacedBlockRecord {
    pub id: String,
    /// 锚点格 (x, y, z)
    pub cell: (i32, i32, i32),
    /// 旋转（90° 倍数；Phase 1 完整版支持旋转后使用）
    pub rot_90: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SaveMeta {
    pub block_count: u32,
    pub completion: f32,
    pub saved_at_unix: u64,
}

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_save_load, env_load_system));
    }
}

/// PHOENIX_LOAD=<path> 启动约 1 秒后导入指定存档（自动化验证/回归用）。
fn env_load_system(
    mut frames: Local<u32>,
    mut done: Local<bool>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed_query: Query<Entity, With<PlacedBlock>>,
) {
    if *done {
        return;
    }
    *frames += 1;
    if *frames < 60 {
        return;
    }
    *done = true;
    let Ok(path) = std::env::var("PHOENIX_LOAD") else {
        return;
    };
    let path = PathBuf::from(path);
    match load_save_from_path(&path) {
        Ok(save) => {
            let loaded = import_save(
                &mut commands,
                &mut stack,
                &library,
                &render,
                &mut blueprint,
                &mut challenge,
                &placed_query,
                save,
            );
            info!(
                "🧪 PHOENIX_LOAD 导入完成：{}（{loaded} 个积木）",
                path.display()
            );
        }
        Err(e) => error!("🧪 PHOENIX_LOAD 导入失败: {e}"),
    }
}

pub fn saves_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("saves")
}

/// 从运行时状态构建存档（纯数据，可测试）。
pub fn build_save(stack: &PlacedBlocks, blueprint: &Blueprint, mode: &str) -> SaveFile {
    let blocks = stack
        .records
        .iter()
        .map(|r| PlacedBlockRecord {
            id: r.def_id.clone(),
            cell: (r.anchor.x, r.anchor.y, r.anchor.z),
            rot_90: r.rot,
        })
        .collect();
    SaveFile {
        version: SAVE_VERSION,
        game_mode: mode.to_string(),
        seed: 0,
        blocks,
        meta: SaveMeta {
            block_count: stack.records.len() as u32,
            completion: blueprint.completion,
            saved_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        },
    }
}

/// 版本迁移链（ADR-006）：未来新增版本在此按版本号串联迁移函数。
pub fn migrate(save: SaveFile) -> Result<SaveFile, String> {
    if save.version > SAVE_VERSION {
        return Err(format!(
            "存档版本 {} 高于当前支持版本 {}",
            save.version, SAVE_VERSION
        ));
    }
    // 当前仅支持 v1，无迁移步骤；未来 v2 起在此按版本号迁移。
    Ok(save)
}

pub fn encode_bincode(save: &SaveFile) -> Result<Vec<u8>, String> {
    bincode::serde::encode_to_vec(save, bincode::config::standard())
        .map_err(|e| format!("bincode 序列化失败: {e}"))
}

pub fn decode_bincode(bytes: &[u8]) -> Result<SaveFile, String> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(save, _)| save)
        .map_err(|e| format!("bincode 反序列化失败: {e}"))
}

/// 原子写入：先写临时文件再重命名，防止中断导致存档损坏。
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("写入临时文件失败: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("重命名失败: {e}"))?;
    Ok(())
}

/// 存档加载（业务逻辑，可测试）：将存档记录应用到世界。
/// 返回加载的积木数量；未知积木 ID 跳过并告警。
pub fn apply_save(
    commands: &mut Commands,
    stack: &mut PlacedBlocks,
    library: &BlockLibrary,
    render: &BlockRenderAssets,
    save: &SaveFile,
) -> usize {
    // 清空世界（由调用方先 despawn 实体，这里只清数据）
    stack.records.clear();
    stack.occupied.clear();
    stack.col_top.clear();

    let mut loaded = 0;
    for rec in &save.blocks {
        let Some(&idx) = library.by_id.get(&rec.id) else {
            warn!("存档包含未知积木 ID「{}」，已跳过", rec.id);
            continue;
        };
        let def = &library.defs[idx];
        let anchor = IVec3::new(rec.cell.0, rec.cell.1, rec.cell.2);
        let rot = rec.rot_90.min(3);
        let cells = footprint_cells(anchor, def, rot);
        let entity = spawn_block_entity(commands, library, render, &rec.id, anchor, rot);
        for c in &cells {
            stack.occupied.insert(*c);
        }
        let h = def.size[1] as i32;
        for (x, z) in footprint_columns(def, anchor.x, anchor.z, rot) {
            let top = stack.col_top.entry((x, z)).or_insert(0);
            *top = (*top).max(anchor.y + h);
        }
        stack.records.push(PlacedRecord {
            entity,
            def_id: rec.id.clone(),
            anchor,
            rot,
            cells,
        });
        loaded += 1;
    }
    stack.revision += 1;
    loaded
}

fn handle_save_load(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed_query: Query<Entity, With<PlacedBlock>>,
) {
    if keys.just_pressed(KeyCode::F5) {
        if let Err(e) = save_slot(&stack, &blueprint) {
            error!("保存失败: {e}");
        }
    }

    if keys.just_pressed(KeyCode::F6) {
        if let Err(e) = export_json(&stack, &blueprint) {
            error!("JSON 导出失败: {e}");
        }
    }

    if keys.just_pressed(KeyCode::F9) {
        if let Err(e) = load_slot(
            &mut commands,
            &mut stack,
            &library,
            &render,
            &mut blueprint,
            &mut challenge,
            &placed_query,
        ) {
            info!("💾 {e}");
        }
    }

    // F7：导出分享副本（时间戳命名，便于分发）
    if keys.just_pressed(KeyCode::F7) {
        if let Err(e) = share_export(&stack, &blueprint) {
            error!("分享导出失败: {e}");
        }
    }

    // F8：导入 saves/ 下最新的 .ptw（分享导入快捷方式）
    if keys.just_pressed(KeyCode::F8) {
        if let Err(e) = import_latest(
            &mut commands,
            &mut stack,
            &library,
            &render,
            &mut blueprint,
            &mut challenge,
            &placed_query,
        ) {
            info!("📂 {e}");
        }
    }
}

/// 读取并迁移存档文件（纯 IO+解析，可测试）。
pub fn load_save_from_path(path: &Path) -> Result<SaveFile, String> {
    std::fs::read(path)
        .map_err(|e| format!("读取存档失败: {e}"))
        .and_then(|bytes| decode_bincode(&bytes))
        .and_then(migrate)
}

/// saves/ 下最新的 .ptw 文件路径（分享导入）。
pub fn latest_ptw(dir: &Path) -> Option<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ptw"))
        .collect();
    files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    files.pop()
}

/// 导入存档到世界（F9 / F8 / 面板 / PHOENIX_LOAD 共用）：
/// 清空现有积木 → 重建 → 重置挑战状态 → 刷新完成度。
pub fn import_save(
    commands: &mut Commands,
    stack: &mut PlacedBlocks,
    library: &BlockLibrary,
    render: &BlockRenderAssets,
    blueprint: &mut Blueprint,
    challenge: &mut Challenge,
    placed_query: &Query<Entity, With<PlacedBlock>>,
    save: SaveFile,
) -> usize {
    for entity in placed_query.iter() {
        commands.entity(entity).despawn();
    }
    let loaded = apply_save(commands, stack, library, render, &save);
    // 导入后重置挑战（避免配额与世界不一致）
    challenge.state = ChallengeState::Idle;
    challenge.rewards.clear();
    challenge.stars = 0;
    if blueprint.active {
        refresh_completion(stack, library, blueprint);
    }
    loaded
}

// ======================================================================
// UI 可复用动作（快捷键 F5-F9 与 lunex 存档面板共用，逻辑单一来源）
// ======================================================================

/// 保存到默认槽位（F5 / lunex 面板「保存」）
pub fn save_slot(stack: &PlacedBlocks, blueprint: &Blueprint) -> Result<(), String> {
    let dir = saves_dir();
    let mode = if blueprint.active { "blueprint" } else { "free" };
    let save = build_save(stack, blueprint, mode);
    encode_bincode(&save).and_then(|bytes| {
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建存档目录失败: {e}"))?;
        write_atomic(&dir.join(SLOT_FILENAME), &bytes)
    })?;
    info!("💾 已保存 {} 个积木 → saves/{}", save.meta.block_count, SLOT_FILENAME);
    Ok(())
}

/// 导出 JSON（F6 / lunex 面板「JSON」）
pub fn export_json(stack: &PlacedBlocks, blueprint: &Blueprint) -> Result<(), String> {
    let dir = saves_dir();
    let mode = if blueprint.active { "blueprint" } else { "free" };
    let save = build_save(stack, blueprint, mode);
    let json = serde_json::to_string_pretty(&save).map_err(|e| format!("JSON 序列化失败: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建存档目录失败: {e}"))?;
    std::fs::write(dir.join("export.json"), json).map_err(|e| format!("JSON 导出失败: {e}"))?;
    info!("📤 已导出 saves/export.json");
    Ok(())
}

/// 导出分享副本（F7 / lunex 面板「分享」；时间戳命名便于分发）
pub fn share_export(stack: &PlacedBlocks, blueprint: &Blueprint) -> Result<(), String> {
    let dir = saves_dir();
    let mode = if blueprint.active { "blueprint" } else { "free" };
    let save = build_save(stack, blueprint, mode);
    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("share_{unix}.ptw"));
    encode_bincode(&save).and_then(|bytes| {
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建存档目录失败: {e}"))?;
        write_atomic(&path, &bytes)
    })?;
    info!("📤 分享存档已导出：{}", path.display());
    Ok(())
}

/// 读取默认槽位（F9 / lunex 面板「加载」）；无存档时返回提示文本
pub fn load_slot(
    commands: &mut Commands,
    stack: &mut PlacedBlocks,
    library: &BlockLibrary,
    render: &BlockRenderAssets,
    blueprint: &mut Blueprint,
    challenge: &mut Challenge,
    placed_query: &Query<Entity, With<PlacedBlock>>,
) -> Result<usize, String> {
    let slot = saves_dir().join(SLOT_FILENAME);
    if !slot.exists() {
        return Err("💾 暂无存档（先保存）".to_string());
    }
    let save = load_save_from_path(&slot)?;
    let loaded = import_save(
        commands,
        stack,
        library,
        render,
        blueprint,
        challenge,
        placed_query,
        save,
    );
    info!("📂 已加载存档（{loaded} 个积木）");
    Ok(loaded)
}

/// 导入 saves/ 下最新的 .ptw（F8 / lunex 面板「导入最新」）
pub fn import_latest(
    commands: &mut Commands,
    stack: &mut PlacedBlocks,
    library: &BlockLibrary,
    render: &BlockRenderAssets,
    blueprint: &mut Blueprint,
    challenge: &mut Challenge,
    placed_query: &Query<Entity, With<PlacedBlock>>,
) -> Result<usize, String> {
    let dir = saves_dir();
    let Some(path) = latest_ptw(&dir) else {
        return Err("saves/ 下没有可导入的 .ptw 存档".to_string());
    };
    let save = load_save_from_path(&path)?;
    let loaded = import_save(
        commands,
        stack,
        library,
        render,
        blueprint,
        challenge,
        placed_query,
        save,
    );
    info!("📂 已导入分享存档 {}（{loaded} 个积木）", path.display());
    Ok(loaded)
}

/// saves/ 下的 .ptw 文件列表（名称 / 积木数 / 修改时间），供存档面板展示。
pub fn save_file_list(dir: &Path) -> Vec<(String, u32, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.filter_map(Result::ok) {
        let path = e.path();
        if path.extension().is_none_or(|x| x != "ptw") {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let blocks = load_save_from_path(&path)
            .ok()
            .map(|s| s.meta.block_count)
            .unwrap_or(0);
        let mtime = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| {
                let secs = t
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let (h, m) = ((secs / 3600) % 24, (secs / 60) % 60);
                format!("{h:02}:{m:02}")
            })
            .unwrap_or_else(|| "--:--".to_string());
        out.push((name, blocks, mtime));
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::block_defs::load_block_library;
    use crate::building::blueprint::load_blueprint;
    use crate::building::placement::PlacedBlocks;

    fn setup() -> (PlacedBlocks, Blueprint, BlockLibrary) {
        (
            PlacedBlocks::default(),
            load_blueprint(),
            load_block_library(),
        )
    }

    #[test]
    fn bincode_round_trip_preserves_save() {
        let (mut stack, bp, lib) = setup();
        let taiji = lib.by_id["taiji"];
        let def = &lib.defs[taiji];
        let anchor = IVec3::new(0, 0, 0);
        let cells = footprint_cells(anchor, def, 0);
        for c in &cells {
            stack.occupied.insert(*c);
        }
        stack.col_top.insert((0, 0), 1);
        stack.records.push(PlacedRecord {
            entity: Entity::PLACEHOLDER,
            def_id: "taiji".to_string(),
            anchor,
            rot: 0,
            cells,
        });

        let save = build_save(&stack, &bp, "free");
        let bytes = encode_bincode(&save).unwrap();
        let back = decode_bincode(&bytes).unwrap();
        assert_eq!(save, back, "bincode 往返应保持存档一致");
        assert_eq!(back.meta.block_count, 1);
        assert_eq!(back.blocks[0].id, "taiji");
        assert_eq!(back.blocks[0].cell, (0, 0, 0));
    }

    #[test]
    fn migrate_accepts_current_version() {
        let (stack, bp, _lib) = setup();
        let save = build_save(&stack, &bp, "free");
        assert!(migrate(save).is_ok());
    }

    #[test]
    fn migrate_rejects_future_version() {
        let (stack, bp, _lib) = setup();
        let mut save = build_save(&stack, &bp, "free");
        save.version = SAVE_VERSION + 99;
        assert!(migrate(save).is_err(), "未来版本应拒绝加载");
    }

    #[test]
    fn load_save_from_path_roundtrip() {
        let (stack, bp, _lib) = setup();
        let save = build_save(&stack, &bp, "free");
        let path = std::env::temp_dir().join("pt_share_test.ptw");
        std::fs::write(&path, encode_bincode(&save).unwrap()).unwrap();
        let loaded = load_save_from_path(&path).unwrap();
        assert_eq!(loaded, save, "路径导入应还原存档");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_save_missing_file_errors() {
        let path = std::env::temp_dir().join("pt_does_not_exist_12345.ptw");
        assert!(load_save_from_path(&path).is_err());
    }

    #[test]
    fn write_verify_import_file() {
        // 生成验证文件供 PHOENIX_LOAD 冒烟测试（saves/ 已 gitignore）
        let (mut stack, bp, lib) = setup();
        let taiji = lib.by_id["taiji"];
        let def = &lib.defs[taiji];
        let anchor = IVec3::new(0, 0, 0);
        let cells = footprint_cells(anchor, def, 0);
        stack.records.push(PlacedRecord {
            entity: Entity::PLACEHOLDER,
            def_id: "taiji".to_string(),
            anchor,
            rot: 0,
            cells,
        });
        let save = build_save(&stack, &bp, "free");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("saves");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("verify_import.ptw"),
            encode_bincode(&save).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn write_verify_plaque_file() {
        // C1 冒烟用：含匾额（biane）的存档，供 PHOENIX_LOAD 验证 Text3d 匾额文字
        let (mut stack, bp, lib) = setup();
        let biane = lib.by_id["biane"];
        let def = &lib.defs[biane];
        let anchor = IVec3::new(0, 0, 0);
        let cells = footprint_cells(anchor, def, 0);
        stack.records.push(PlacedRecord {
            entity: Entity::PLACEHOLDER,
            def_id: "biane".to_string(),
            anchor,
            rot: 0,
            cells,
        });
        let save = build_save(&stack, &bp, "free");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("saves");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("verify_plaque.ptw"),
            encode_bincode(&save).unwrap(),
        )
        .unwrap();
    }
}

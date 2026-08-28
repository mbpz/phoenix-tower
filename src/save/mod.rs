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
        app.add_systems(Update, handle_save_load);
    }
}

fn saves_dir() -> PathBuf {
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
    let dir = saves_dir();
    let mode = if blueprint.active { "blueprint" } else { "free" };

    if keys.just_pressed(KeyCode::F5) {
        let save = build_save(&stack, &blueprint, mode);
        match encode_bincode(&save).and_then(|bytes| {
            std::fs::create_dir_all(&dir).map_err(|e| format!("创建存档目录失败: {e}"))?;
            write_atomic(&dir.join(SLOT_FILENAME), &bytes)
        }) {
            Ok(()) => info!(
                "💾 已保存 {} 个积木 → saves/{}",
                save.meta.block_count, SLOT_FILENAME
            ),
            Err(e) => error!("保存失败: {e}"),
        }
    }

    if keys.just_pressed(KeyCode::F6) {
        let save = build_save(&stack, &blueprint, mode);
        match serde_json::to_string_pretty(&save) {
            Ok(json) => {
                if let Err(e) = std::fs::write(dir.join("export.json"), json) {
                    error!("JSON 导出失败: {e}");
                } else {
                    info!("📤 已导出 saves/export.json");
                }
            }
            Err(e) => error!("JSON 序列化失败: {e}"),
        }
    }

    if keys.just_pressed(KeyCode::F9) {
        let result = std::fs::read(dir.join(SLOT_FILENAME))
            .map_err(|e| format!("读取存档失败: {e}"))
            .and_then(|bytes| decode_bincode(&bytes))
            .and_then(migrate);
        match result {
            Ok(save) => {
                for entity in placed_query.iter() {
                    commands.entity(entity).despawn();
                }
                let loaded = apply_save(&mut commands, &mut stack, &library, &render, &save);
                // 读档后重置挑战（避免配额与世界不一致）
                challenge.state = ChallengeState::Idle;
                challenge.rewards.clear();
                challenge.stars = 0;
                if blueprint.active {
                    refresh_completion(&stack, &library, &mut blueprint);
                }
                info!("📂 已加载存档（{} 个积木）", loaded);
            }
            Err(e) => error!("加载失败: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::block_defs::load_block_library;
    use crate::building::blueprint::load_blueprint;
    use crate::building::placement::PlacedBlocks;

    fn setup() -> (PlacedBlocks, Blueprint, BlockLibrary) {
        (PlacedBlocks::default(), load_blueprint(), load_block_library())
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
}

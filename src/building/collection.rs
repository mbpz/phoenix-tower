//! 图鉴与成就（B-18）：PRD §3.4 收集系统。
//!
//! - 图鉴：蓝图模式下正确放置某积木 → 解锁该部件条目（名称/尺寸/层级/文化描述）
//! - 稀有：挑战胜利解锁「鎏金宝顶」稀有条目
//! - 成就（5 项）：
//!   1. 初见黄鹤楼：首次完成复原
//!   2. 一气呵成：完成复原且全程未撤销
//!   3. 挑战胜者：限时内完成挑战
//!   4. 极速复原：挑战 3 星
//!   5. 广厦千万：累计搭建满 30 块积木
//! - 持久化：saves/progress.json（解锁状态跨启动保留）
//!
//! 检测逻辑为纯函数（Collection::update / scan_placed），单元测试覆盖。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::building::block_defs::BlockLibrary;
use crate::building::challenge::ChallengeState;
use crate::building::placement::PlacedBlocks;

// ---------- 成就定义 ----------

pub const ACH_FIRST: &str = "first_completion";
pub const ACH_FLAWLESS: &str = "flawless";
pub const ACH_WINNER: &str = "challenge_winner";
pub const ACH_PERFECT: &str = "challenge_perfect";
pub const ACH_BUILDER: &str = "big_builder";

/// 成就元数据（id, 名称, 描述）
pub fn achievement_defs() -> [(&'static str, &'static str, &'static str); 5] {
    [
        (ACH_FIRST, "初见黄鹤楼", "首次完成黄鹤楼复原"),
        (ACH_FLAWLESS, "一气呵成", "完成复原且全程未使用撤销"),
        (ACH_WINNER, "挑战胜者", "在限时内完成一次挑战"),
        (ACH_PERFECT, "极速复原", "挑战获得 3 星评价"),
        (ACH_BUILDER, "广厦千万", "累计搭建满 30 块积木"),
    ]
}

pub fn achievement_name(id: &str) -> &'static str {
    achievement_defs()
        .iter()
        .find(|(i, _, _)| *i == id)
        .map(|(_, n, _)| *n)
        .unwrap_or("未知成就")
}

// ---------- 运行时状态 ----------

/// 收藏系统运行态（图鉴 + 稀有 + 成就 + 检测状态）。
#[derive(Resource, Default)]
pub struct Collection {
    /// 已解锁图鉴条目（积木 ID）
    pub codex: HashSet<String>,
    /// 已解锁稀有条目
    pub rare: HashSet<String>,
    /// 已解锁成就（成就 ID）
    pub achievements: HashSet<String>,
    /// 检测状态：蓝图完成上升沿
    blueprint_completed_seen: bool,
    /// 检测状态：蓝图激活上升沿（重置"未撤销"标记）
    blueprint_active_prev: bool,
    /// 本次蓝图运行是否用过撤销
    pub undo_used_this_run: bool,
    /// 检测状态：挑战胜利上升沿
    challenge_won_seen: bool,
    /// 上次处理的世界版本（revision 门控，B-20）
    last_revision: u64,
    /// 有待写入磁盘的变更
    dirty: bool,
}

/// 一次 update 产生的新解锁（供日志与持久化）。
#[derive(Default, Debug)]
pub struct Updates {
    pub codex: Vec<String>,
    pub rare: Vec<String>,
    pub achievements: Vec<String>,
}

impl Collection {
    /// 纯函数式检测：给定世界事实，推进状态并返回新解锁。
    /// `revision` 为世界变更版本号：不变时跳过 O(n) 的图鉴扫描（B-20）。
    pub fn update(
        &mut self,
        stack: &PlacedBlocks,
        blueprint_completed: bool,
        blueprint_active: bool,
        challenge_state: ChallengeState,
        challenge_stars: u8,
        undo_pressed: bool,
        revision: u64,
    ) -> Updates {
        let mut updates = Updates::default();

        // 图鉴：蓝图模式下已放置积木全部解锁（严格模式保证放置正确）
        // revision 门控：世界未变更时跳过（放置/撤销/重做/读档会递增 revision）
        if revision != self.last_revision {
            self.last_revision = revision;
            if blueprint_active {
                for r in &stack.records {
                    if self.codex.insert(r.def_id.clone()) {
                        updates.codex.push(r.def_id.clone());
                    }
                }
            }
        }

        // 蓝图激活上升沿：重置"未撤销"标记（每次进入蓝图重新计数）
        if blueprint_active && !self.blueprint_active_prev {
            self.undo_used_this_run = false;
        }
        if undo_pressed && blueprint_active {
            self.undo_used_this_run = true;
        }
        self.blueprint_active_prev = blueprint_active;

        // 蓝图完成（上升沿）
        if blueprint_completed && !self.blueprint_completed_seen {
            self.unlock_achievement(ACH_FIRST, &mut updates);
            if !self.undo_used_this_run {
                self.unlock_achievement(ACH_FLAWLESS, &mut updates);
            }
        }
        self.blueprint_completed_seen = blueprint_completed;

        // 挑战胜利（上升沿）：胜者 + 3 星 + 稀有奖励
        if challenge_state == ChallengeState::Won && !self.challenge_won_seen {
            self.unlock_achievement(ACH_WINNER, &mut updates);
            if challenge_stars >= 3 {
                self.unlock_achievement(ACH_PERFECT, &mut updates);
            }
            if self.rare.insert("baoding".to_string()) {
                updates.rare.push("baoding".to_string());
            }
        }
        self.challenge_won_seen = challenge_state == ChallengeState::Won;

        // 大体积
        if stack.records.len() >= 30 {
            self.unlock_achievement(ACH_BUILDER, &mut updates);
        }

        if !updates.codex.is_empty() || !updates.rare.is_empty() || !updates.achievements.is_empty()
        {
            self.dirty = true;
        }
        updates
    }

    fn unlock_achievement(&mut self, id: &str, updates: &mut Updates) {
        if self.achievements.insert(id.to_string()) {
            updates.achievements.push(id.to_string());
        }
    }

    /// 是否有待持久化变更。
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }
}

// ---------- 持久化（saves/progress.json） ----------

#[derive(Serialize, Deserialize)]
struct ProgressFile {
    codex: Vec<String>,
    rare: Vec<String>,
    achievements: Vec<String>,
}

fn progress_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("saves/progress.json")
}

/// 保存到默认路径。
pub fn save_progress(collection: &Collection) -> Result<(), String> {
    save_progress_to(&progress_path(), collection)
}

/// 保存到指定路径（可测试）。
pub fn save_progress_to(path: &Path, collection: &Collection) -> Result<(), String> {
    let mut codex: Vec<_> = collection.codex.iter().cloned().collect();
    let mut rare: Vec<_> = collection.rare.iter().cloned().collect();
    let mut achievements: Vec<_> = collection.achievements.iter().cloned().collect();
    codex.sort();
    rare.sort();
    achievements.sort();
    let file = ProgressFile {
        codex,
        rare,
        achievements,
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| format!("序列化失败: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    crate::save::write_atomic(path, json.as_bytes())
}

/// 从默认路径加载（缺失文件 → 空收藏）。
pub fn load_progress() -> Collection {
    load_progress_from(&progress_path())
}

/// 从指定路径加载（可测试）。
pub fn load_progress_from(path: &Path) -> Collection {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Collection::default();
    };
    match serde_json::from_str::<ProgressFile>(&text) {
        Ok(file) => Collection {
            codex: file.codex.into_iter().collect(),
            rare: file.rare.into_iter().collect(),
            achievements: file.achievements.into_iter().collect(),
            ..Collection::default()
        },
        Err(e) => {
            warn!("进度文件解析失败 {}: {e}", path.display());
            Collection::default()
        }
    }
}

// ---------- 知识卡片智能提示（PRD §3.3） ----------

/// 知识卡片（文化小知识）。
#[derive(Clone, Debug)]
pub struct KnowledgeCard {
    pub name: String,
    pub desc: String,
}

/// 智能提示状态：蓝图模式长时间停顿（>12s）时弹出已解锁部件的知识卡片。
#[derive(Resource)]
pub struct KnowledgeHints {
    pub enabled: bool,
    /// 上次放置动作时间（秒）
    pub last_action_secs: f32,
    pub card: Option<KnowledgeCard>,
    pub card_expires_at: f32,
    /// 卡片轮换索引（循环展示不同部件）
    pub next_idx: usize,
}

impl Default for KnowledgeHints {
    fn default() -> Self {
        Self {
            enabled: true,
            last_action_secs: 0.0,
            card: None,
            card_expires_at: 0.0,
            next_idx: 0,
        }
    }
}

/// 选取下一张知识卡片（纯函数，可测试）：在已解锁图鉴中轮换。
pub fn pick_next_card(
    unlocked: &HashSet<String>,
    library: &BlockLibrary,
    start: usize,
) -> Option<(KnowledgeCard, usize)> {
    let mut ids: Vec<&String> = unlocked.iter().collect();
    ids.sort();
    if ids.is_empty() {
        return None;
    }
    let idx = start % ids.len();
    let def = &library.defs[library.by_id[ids[idx]]];
    let card = KnowledgeCard {
        name: def.name.clone(),
        desc: def.description.clone(),
    };
    Some((card, (idx + 1) % ids.len()))
}

/// 智能提示系统：K 键开关；放置动作重置空闲计时；停顿 >12s 弹卡片（8s 后消失）。
fn knowledge_hint_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<crate::building::blueprint::Blueprint>,
    collection: Res<Collection>,
    library: Res<BlockLibrary>,
    mut hints: ResMut<KnowledgeHints>,
    mut prev_revision: Local<u64>,
) {
    if keys.just_pressed(KeyCode::KeyK) {
        hints.enabled = !hints.enabled;
        if !hints.enabled {
            hints.card = None;
        }
        info!("📖 知识提示{}", if hints.enabled { "开启" } else { "关闭" });
    }
    if !hints.enabled {
        return;
    }

    // 放置/撤销等世界变更 → 重置空闲计时
    if stack.revision != *prev_revision {
        *prev_revision = stack.revision;
        hints.last_action_secs = time.elapsed_secs();
        hints.card = None;
    }

    if !blueprint.active {
        return;
    }

    let idle = time.elapsed_secs() - hints.last_action_secs;
    if idle > 12.0 && hints.card.is_none() {
        if let Some((card, next)) = pick_next_card(&collection.codex, &library, hints.next_idx) {
            hints.next_idx = next;
            hints.card = Some(card);
            hints.card_expires_at = time.elapsed_secs() + 8.0;
        }
    }
    if hints.card.is_some() && time.elapsed_secs() > hints.card_expires_at {
        hints.card = None;
    }
}

// ---------- 系统 ----------

pub struct CollectionPlugin;

impl Plugin for CollectionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_progress())
            .insert_resource(KnowledgeHints::default())
            .add_systems(Update, (collection_system, knowledge_hint_system));
    }
}

fn collection_system(
    keys: Res<ButtonInput<KeyCode>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<crate::building::blueprint::Blueprint>,
    challenge: Res<crate::building::challenge::Challenge>,
    mut collection: ResMut<Collection>,
) {
    let modifier = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    let undo_pressed =
        keys.just_pressed(KeyCode::Backspace) || (modifier && keys.just_pressed(KeyCode::KeyZ));

    let updates = collection.update(
        &stack,
        blueprint.completed,
        blueprint.active,
        challenge.state,
        challenge.stars,
        undo_pressed,
        stack.revision,
    );

    for id in &updates.codex {
        info!("📖 图鉴解锁：{id}");
    }
    for id in &updates.rare {
        info!("✨ 稀有部件解锁：{id}");
    }
    for id in &updates.achievements {
        info!("🏅 成就达成：{}", achievement_name(id));
    }

    if collection.is_dirty() {
        if let Err(e) = save_progress(&collection) {
            error!("进度保存失败: {e}");
        }
        collection.mark_saved();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::block_defs::load_block_library;
    use crate::building::placement::{PlacedBlocks, PlacedRecord};

    fn stack_with(n: usize) -> PlacedBlocks {
        let lib = load_block_library();
        let mut stack = PlacedBlocks::default();
        let taiji = &lib.defs[lib.by_id["taiji"]];
        for i in 0..n {
            let anchor = IVec3::new(i as i32 * 2, 0, 0);
            let cells = crate::building::placement::footprint_cells(anchor, taiji, 0);
            for c in &cells {
                stack.occupied.insert(*c);
            }
            stack.records.push(PlacedRecord {
                entity: Entity::PLACEHOLDER,
                def_id: "taiji".to_string(),
                anchor,
                rot: 0,
                cells,
            });
        }
        stack
    }

    #[test]
    fn first_completion_unlocks() {
        let mut col = Collection::default();
        let stack = stack_with(5);
        let u = col.update(&stack, true, true, ChallengeState::Idle, 0, false, 1);
        assert!(u.achievements.contains(&ACH_FIRST.to_string()));
        assert!(
            u.achievements.contains(&ACH_FLAWLESS.to_string()),
            "未撤销时应解锁一气呵成"
        );
    }

    #[test]
    fn flawless_requires_no_undo() {
        let mut col = Collection::default();
        // 用了一次撤销后再完成
        col.update(
            &stack_with(1),
            false,
            true,
            ChallengeState::Idle,
            0,
            true,
            1,
        );
        let stack = stack_with(5);
        let u = col.update(&stack, true, true, ChallengeState::Idle, 0, false, 1);
        assert!(u.achievements.contains(&ACH_FIRST.to_string()));
        assert!(
            !u.achievements.contains(&ACH_FLAWLESS.to_string()),
            "用过撤销不应解锁一气呵成"
        );
    }

    #[test]
    fn challenge_win_unlocks_and_rare() {
        let mut col = Collection::default();
        let stack = stack_with(0);
        let u = col.update(&stack, false, true, ChallengeState::Won, 3, false, 1);
        assert!(u.achievements.contains(&ACH_WINNER.to_string()));
        assert!(u.achievements.contains(&ACH_PERFECT.to_string()));
        assert_eq!(u.rare, vec!["baoding".to_string()]);
    }

    #[test]
    fn codex_unlocks_blueprint_placements() {
        let mut col = Collection::default();
        let stack = stack_with(1); // 1 块 taiji
        let u = col.update(&stack, false, true, ChallengeState::Idle, 0, false, 1);
        assert_eq!(u.codex, vec!["taiji".to_string()]);
        // 非蓝图模式不放图鉴
        let mut col2 = Collection::default();
        let u2 = col2.update(&stack, false, false, ChallengeState::Idle, 0, false, 1);
        assert!(u2.codex.is_empty());
    }

    #[test]
    fn builder_achievement_at_30() {
        let mut col = Collection::default();
        let u = col.update(
            &stack_with(29),
            false,
            false,
            ChallengeState::Idle,
            0,
            false,
            1,
        );
        assert!(!u.achievements.contains(&ACH_BUILDER.to_string()));
        let u2 = col.update(
            &stack_with(30),
            false,
            false,
            ChallengeState::Idle,
            0,
            false,
            1,
        );
        assert!(u2.achievements.contains(&ACH_BUILDER.to_string()));
    }

    #[test]
    fn progress_roundtrip() {
        let path = std::env::temp_dir().join("pt_progress_test.json");
        let _ = std::fs::remove_file(&path);
        let mut col = Collection::default();
        col.codex.insert("taiji".to_string());
        col.achievements.insert(ACH_FIRST.to_string());
        col.rare.insert("baoding".to_string());
        save_progress_to(&path, &col).unwrap();

        let loaded = load_progress_from(&path);
        assert!(loaded.codex.contains("taiji"));
        assert!(loaded.achievements.contains(ACH_FIRST));
        assert!(loaded.rare.contains("baoding"));
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod knowledge_tests {
    use super::*;
    use crate::building::block_defs::load_block_library;

    #[test]
    fn pick_card_cycles_unlocked() {
        let lib = load_block_library();
        let mut unlocked = HashSet::new();
        unlocked.insert("taiji".to_string());
        unlocked.insert("hongzhu".to_string());
        let (c1, n1) = pick_next_card(&unlocked, &lib, 0).unwrap();
        let (c2, n2) = pick_next_card(&unlocked, &lib, n1).unwrap();
        assert_ne!(c1.name, c2.name, "应轮换到不同部件");
        assert!(!c1.desc.is_empty());
        assert_eq!(n2, 0, "两元素循环后回到 0");
    }

    #[test]
    fn pick_card_empty_returns_none() {
        let lib = load_block_library();
        assert!(pick_next_card(&HashSet::new(), &lib, 0).is_none());
    }
}

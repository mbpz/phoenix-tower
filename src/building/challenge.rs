//! 挑战模式（B-16）：限时复原黄鹤楼 + 材料配额 + 星级评价。
//!
//! 规则（REVIEW P2：以可量化规则替代装饰性"风向"）：
//! - 限时：倒计时归零即失败
//! - 限料：每种积木有配额（放置扣减 / 撤销退返 / 重做再校验）
//! - 限旋转：rotation_locked 时强制 0°（R 键无效）
//! - 星级：完成 1★ + 剩余时间 > 30% 加 1★ + 剩余材料 > 0 加 1★
//! - 完成奖励：稀有积木解锁（MVP 占位，联动 B-18 图鉴成就）
//!
//! 挑战定义数据驱动：resources/challenges/*.ron

use std::collections::HashMap;

use bevy::prelude::*;
use serde::Deserialize;

use crate::building::blueprint::{blueprint_from_def, Blueprint, BlueprintLibrary};
use crate::building::placement::{PlacedBlock, PlacedBlocks};

/// 挑战定义（resources/challenges/*.ron）。
// id 供 Phase 2 多挑战选择（主题包 B-24）使用，当前未消费，故 allow(dead_code)。
#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct ChallengeDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// 使用的蓝图 ID（当前仅一个蓝图，读取时校验存在）
    pub blueprint_id: String,
    /// 限时（秒）
    pub time_limit_secs: u32,
    /// 限旋转：true 时强制 0°
    pub rotation_locked: bool,
    /// 材料配额：(block_id, 数量)
    pub quota: Vec<(String, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeState {
    /// 未开始
    Idle,
    /// 进行中
    Active,
    /// 限时内完成
    Won,
    /// 时间耗尽
    Failed,
}

/// 挑战运行态资源。
#[derive(Resource)]
pub struct Challenge {
    pub def: ChallengeDef,
    pub state: ChallengeState,
    pub time_left: f32,
    pub quota_left: HashMap<String, u32>,
    pub stars: u8,
    /// 奖励（MVP 占位：解锁稀有积木名）
    pub rewards: Vec<String>,
}

impl Challenge {
    pub fn is_active(&self) -> bool {
        self.state == ChallengeState::Active
    }

    /// 是否可放置该积木（挑战外始终允许；挑战内校验配额）。
    pub fn can_place(&self, def_id: &str) -> bool {
        !self.is_active() || self.quota_left.get(def_id).copied().unwrap_or(0) > 0
    }

    pub(crate) fn consume(&mut self, def_id: &str) {
        if let Some(q) = self.quota_left.get_mut(def_id) {
            *q = q.saturating_sub(1);
        }
    }

    pub(crate) fn refund(&mut self, def_id: &str) {
        *self.quota_left.entry(def_id.to_string()).or_insert(0) += 1;
    }

    /// 已用 / 总配额（用于 HUD 与面板展示）。
    pub fn quota_used_total(&self) -> (u32, u32) {
        let total: u32 = self.def.quota.iter().map(|(_, n)| n).sum();
        let used: u32 = self
            .def
            .quota
            .iter()
            .map(|(id, total_q)| {
                total_q - self.quota_left.get(id).copied().unwrap_or(0).min(*total_q)
            })
            .sum();
        (used, total)
    }
}

pub struct ChallengePlugin;

impl Plugin for ChallengePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_challenge())
            .add_systems(Update, (challenge_key_start, challenge_tick).chain());
    }
}

/// 从 resources/challenges/*.ron 加载（当前取第一个）。
pub fn load_challenge() -> Challenge {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/challenges");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("无法读取挑战目录 {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    let Some(path) = files.first() else {
        panic!("resources/challenges/ 下未找到任何挑战 (.ron)");
    };
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("读取挑战失败 {}: {e}", path.display()));
    let def: ChallengeDef =
        ron::from_str(&text).unwrap_or_else(|e| panic!("解析挑战失败 {}: {e}", path.display()));

    let quota_left = def.quota.iter().cloned().collect();
    Challenge {
        def,
        state: ChallengeState::Idle,
        time_left: 0.0,
        quota_left,
        stars: 0,
        rewards: Vec::new(),
    }
}

/// 星级计算（纯函数，可测试）：完成 1★ + 剩余时间比 > 0.3 加 1★ + 剩余材料 > 0 加 1★。
pub fn compute_stars(time_ratio_left: f32, quota_surplus: u32) -> u8 {
    let mut stars = 1;
    if time_ratio_left > 0.3 {
        stars += 1;
    }
    if quota_surplus > 0 {
        stars += 1;
    }
    stars.min(3)
}

/// 启动挑战：清空世界、按挑战引用选择蓝图主题、强制蓝图模式、重置配额与计时。
/// （C 键与面板按钮共用此入口）
pub fn start_challenge(
    commands: &mut Commands,
    stack: &mut PlacedBlocks,
    blueprint: &mut Blueprint,
    blueprint_library: &mut BlueprintLibrary,
    challenge: &mut Challenge,
    placed_query: &Query<Entity, With<PlacedBlock>>,
) {
    for entity in placed_query.iter() {
        commands.entity(entity).despawn();
    }
    stack.records.clear();
    stack.occupied.clear();
    stack.col_top.clear();
    stack.redo.clear();
    stack.revision += 1;

    // 挑战按 blueprint_id 选择主题（B-24：主题包切换后挑战仍对应正确蓝图）
    if let Some(idx) = blueprint_library.select_by_id(&challenge.def.blueprint_id) {
        *blueprint = blueprint_from_def(blueprint_library.current_def());
        debug_assert_eq!(idx, blueprint_library.current);
    } else {
        warn!(
            "挑战「{}」引用的蓝图 {} 未加载",
            challenge.def.name, challenge.def.blueprint_id
        );
    }

    blueprint.active = true;
    blueprint.completed = false;
    blueprint.completion = 0.0;

    challenge.state = ChallengeState::Active;
    challenge.time_left = challenge.def.time_limit_secs as f32;
    challenge.quota_left = challenge.def.quota.iter().cloned().collect();
    challenge.stars = 0;
    challenge.rewards.clear();

    info!(
        "🏆 挑战开始：{}（限时 {}s，材料 {} 种）",
        challenge.def.name,
        challenge.def.time_limit_secs,
        challenge.quota_left.len()
    );
}

/// C 键启动 / 重新开始挑战。
fn challenge_key_start(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    mut blueprint: ResMut<Blueprint>,
    mut blueprint_library: ResMut<BlueprintLibrary>,
    mut challenge: ResMut<Challenge>,
    placed_query: Query<Entity, With<PlacedBlock>>,
) {
    if keys.just_pressed(KeyCode::KeyC) {
        start_challenge(
            &mut commands,
            &mut stack,
            &mut blueprint,
            &mut blueprint_library,
            &mut challenge,
            &placed_query,
        );
    }
}

/// 挑战计时、强制蓝图模式、完成检测与星级结算。
fn challenge_tick(
    time: Res<Time>,
    mut challenge: ResMut<Challenge>,
    mut blueprint: ResMut<Blueprint>,
) {
    if !challenge.is_active() {
        return;
    }

    // 挑战期间强制蓝图模式（防止 M 键切走）
    if !blueprint.active {
        blueprint.active = true;
    }

    // 倒计时
    challenge.time_left -= time.delta_secs();
    if challenge.time_left <= 0.0 {
        challenge.time_left = 0.0;
        challenge.state = ChallengeState::Failed;
        info!("⏱ 挑战失败：时间耗尽，按 C 重试");
        return;
    }

    // 完成检测（blueprint.completed 由放置系统在 ≥95% 时置位）
    if blueprint.completed {
        let time_ratio = challenge.time_left / challenge.def.time_limit_secs as f32;
        let surplus = challenge.quota_left.values().sum::<u32>();
        challenge.stars = compute_stars(time_ratio, surplus);
        challenge.state = ChallengeState::Won;
        // 稀有积木奖励（MVP 占位；联动 B-18 图鉴成就）
        challenge.rewards.push("baoding".to_string());
        info!(
            "🏆 挑战完成「{}」！{}{}",
            challenge.def.name,
            "★".repeat(challenge.stars as usize),
            if challenge.stars == 3 {
                " 完美！"
            } else {
                ""
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_formula() {
        assert_eq!(compute_stars(0.5, 2), 3, "时间+材料富余 → 3 星");
        assert_eq!(compute_stars(0.4, 0), 2, "仅时间富余 → 2 星");
        assert_eq!(compute_stars(0.1, 0), 1, "勉强完成 → 1 星");
        assert_eq!(compute_stars(0.2, 5), 2, "仅材料富余 → 2 星");
    }

    #[test]
    fn challenge_ron_loads() {
        let c = load_challenge();
        assert_eq!(c.def.id, "speed_restoration");
        assert!(!c.def.quota.is_empty());
        assert!(c.def.time_limit_secs > 0);
        assert!(!c.is_active());
    }

    #[test]
    fn quota_consume_refund_roundtrip() {
        let mut c = load_challenge();
        c.state = ChallengeState::Active;
        assert!(c.can_place("taiji"));
        c.consume("taiji");
        c.consume("taiji");
        // 配额 1 → 用掉 2 次后为 0，不可再放
        assert!(!c.can_place("taiji"));
        c.refund("taiji");
        assert!(c.can_place("taiji"));
    }

    #[test]
    fn non_quota_block_rejected_during_challenge() {
        let mut c = load_challenge();
        assert!(!c.quota_left.contains_key("huaping"));
        c.state = ChallengeState::Active;
        assert!(!c.can_place("huaping"), "挑战内非配额积木不可放置");
        // 挑战外可放任意
        c.state = ChallengeState::Idle;
        assert!(c.can_place("huaping"));
    }
}

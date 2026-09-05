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
    /// 排序/默认选择（order 最小者为默认挑战）
    pub order: u32,
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
        let (challenge, library) = load_challenge_library();
        app.insert_resource(challenge)
            .insert_resource(library)
            .add_systems(Update, (challenge_key_start, challenge_tick).chain());
    }
}

/// 从 resources/challenges/*.ron 加载（当前取第一个）。
/// 挑战库（B-16 完善）：全部挑战 + 当前选择；新增挑战只需加 RON。
#[derive(Resource)]
pub struct ChallengeLibrary {
    pub defs: Vec<ChallengeDef>,
    pub current: usize,
}

impl ChallengeLibrary {
    pub fn current_def(&self) -> &ChallengeDef {
        &self.defs[self.current]
    }

    /// 按 ID 选择（挑战引用预留；当前面板用索引选择）
    #[allow(dead_code)]
    pub fn select_by_id(&mut self, id: &str) -> Option<usize> {
        let idx = self.defs.iter().position(|d| d.id == id)?;
        self.current = idx;
        Some(idx)
    }
}

/// 由定义构建挑战运行态。
fn challenge_from_def(def: &ChallengeDef) -> Challenge {
    Challenge {
        def: def.clone(),
        state: ChallengeState::Idle,
        time_left: 0.0,
        quota_left: def.quota.iter().cloned().collect(),
        stars: 0,
        rewards: Vec::new(),
    }
}

/// 加载全部挑战（按文件名字典序；首个为默认）。
pub fn load_challenge_library() -> (Challenge, ChallengeLibrary) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/challenges");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("无法读取挑战目录 {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    if files.is_empty() {
        panic!("resources/challenges/ 下未找到任何挑战 (.ron)");
    }
    let mut defs: Vec<ChallengeDef> = files
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("读取挑战失败 {}: {e}", path.display()));
            ron::from_str(&text).unwrap_or_else(|e| panic!("解析挑战失败 {}: {e}", path.display()))
        })
        .collect();
    defs.sort_by_key(|d| d.order);
    let challenge = challenge_from_def(&defs[0]);
    let library = ChallengeLibrary { defs, current: 0 };
    (challenge, library)
}

/// 切换挑战（面板选择）：重置为对应定义的运行态。
pub fn select_challenge(challenge: &mut Challenge, library: &mut ChallengeLibrary, idx: usize) {
    if idx >= library.defs.len() {
        return;
    }
    library.current = idx;
    *challenge = challenge_from_def(&library.defs[idx]);
}

/// 兼容入口：加载默认挑战（测试/单挑战场景）。
#[allow(dead_code)]
pub fn load_challenge() -> Challenge {
    load_challenge_library().0
}

/// 星级计算（纯函数，可测试）：完成 1★ + 剩余时间比 > 0.5 加 1★ + > 0.3 再加 1★。
/// 注：原"材料富余"星在严格蓝图中结构性不可达成（放置被限制在蓝图格内），
/// 故第 3 星改为时间充裕度（用户可冲刺速通获得 3 星）。
pub fn compute_stars(time_ratio_left: f32, _quota_surplus: u32) -> u8 {
    if time_ratio_left > 0.5 {
        3
    } else if time_ratio_left > 0.3 {
        2
    } else {
        1
    }
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
    stack.clear();

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

    // 倒计时（time_limit_secs == 0 表示不限时）
    if challenge.def.time_limit_secs > 0 {
        challenge.time_left -= time.delta_secs();
        if challenge.time_left <= 0.0 {
            challenge.time_left = 0.0;
            challenge.state = ChallengeState::Failed;
            info!("⏱ 挑战失败：时间耗尽，按 C 重试");
            return;
        }
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
        assert_eq!(compute_stars(0.6, 0), 3, "时间 >50% → 3 星");
        assert_eq!(compute_stars(0.5, 2), 2, "时间 ≤50% → 2 星（富余不参与）");
        assert_eq!(compute_stars(0.4, 0), 2, "时间 >30% → 2 星");
        assert_eq!(compute_stars(0.1, 0), 1, "勉强完成 → 1 星");
        assert_eq!(compute_stars(0.2, 5), 1, "时间不足，材料富余不计星");
    }

    #[test]
    fn library_loads_all_challenges() {
        let (_c, lib) = load_challenge_library();
        assert!(lib.defs.len() >= 2, "应加载全部挑战（含新增）");
        let ids: Vec<&str> = lib.defs.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"speed_restoration"));
    }

    #[test]
    fn select_challenge_switches() {
        let (mut c, mut lib) = load_challenge_library();
        assert_eq!(lib.select_by_id("speed_demon"), Some(1));
        let idx = lib.current;
        select_challenge(&mut c, &mut lib, idx);
        assert_eq!(c.def.id, "speed_demon");
        assert_eq!(c.state, ChallengeState::Idle, "切换后应为未开始");
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
        assert!(c.can_place("datiji"));
        c.consume("datiji");
        c.consume("datiji");
        // 配额 1 → 用掉 2 次后为 0，不可再放
        assert!(!c.can_place("datiji"));
        c.refund("datiji");
        assert!(c.can_place("datiji"));
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

    // Headless integration fixture: run the same C-key/start and tick systems as
    // gameplay, with deterministic Time and no renderer or wall-clock plugin.
    fn lifecycle_app() -> App {
        use crate::building::blueprint::BlueprintDef;

        let old = BlueprintDef {
            id: "old-theme".into(),
            name: "Old theme".into(),
            order: 0,
            cells: vec![(9, 0, 0, "taiji".into())],
        };
        let target = BlueprintDef {
            id: "challenge-theme".into(),
            name: "Challenge theme".into(),
            order: 1,
            cells: vec![(0, 0, 0, "taiji".into())],
        };
        let def = ChallengeDef {
            id: "lifecycle".into(),
            order: 0,
            name: "Lifecycle fixture".into(),
            description: String::new(),
            blueprint_id: target.id.clone(),
            time_limit_secs: 100,
            rotation_locked: true,
            quota: vec![("taiji".into(), 2)],
        };
        let mut app = App::new();
        app.insert_resource(blueprint_from_def(&old))
            .insert_resource(BlueprintLibrary {
                defs: vec![old, target],
                current: 0,
            })
            .insert_resource(challenge_from_def(&def))
            .insert_resource(Time::<()>::default())
            .init_resource::<PlacedBlocks>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (challenge_key_start, challenge_tick).chain());
        app
    }

    fn lifecycle_step(app: &mut App, elapsed_secs: f32, restart: bool) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(elapsed_secs));
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.reset_all();
        if restart {
            keys.press(KeyCode::KeyC);
        }
        app.update();
    }

    fn place_fixture(app: &mut App, cell: IVec3) -> Entity {
        let entity = app.world_mut().spawn(PlacedBlock).id();
        app.world_mut().resource_mut::<PlacedBlocks>().place(
            crate::building::placement::PlacedRecord {
                entity,
                def_id: "taiji".into(),
                anchor: cell,
                rot: 0,
                cells: vec![cell],
            },
        );
        entity
    }

    fn complete_fixture_blueprint(app: &mut App) {
        place_fixture(app, IVec3::ZERO);
        let library = crate::building::block_defs::load_block_library();
        app.world_mut()
            .resource_scope(|world, mut blueprint: Mut<Blueprint>| {
                crate::building::placement::refresh_completion(
                    world.resource::<PlacedBlocks>(),
                    &library,
                    &mut blueprint,
                );
            });
        let blueprint = app.world().resource::<Blueprint>();
        assert!(blueprint.completed);
        assert!((blueprint.completion - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn lifecycle_start_clears_ecs_history_and_resets_target_blueprint() {
        let mut app = lifecycle_app();
        let tracked = place_fixture(&mut app, IVec3::X);
        let undone = place_fixture(&mut app, IVec3::Y);
        app.world_mut().resource_mut::<PlacedBlocks>().undo();
        app.world_mut().despawn(undone);
        let untracked = app.world_mut().spawn(PlacedBlock).id();
        let unrelated = app.world_mut().spawn_empty().id();
        {
            let mut blueprint = app.world_mut().resource_mut::<Blueprint>();
            blueprint.completed = true;
            blueprint.completion = 1.0;
        }
        {
            let mut challenge = app.world_mut().resource_mut::<Challenge>();
            challenge.state = ChallengeState::Won;
            challenge.time_left = 3.0;
            challenge.quota_left.clear();
            challenge.stars = 3;
            challenge.rewards.push("old-reward".into());
        }
        let revision = app.world().resource::<PlacedBlocks>().revision;
        lifecycle_step(&mut app, 0.0, true);

        assert!(app.world().get_entity(tracked).is_err());
        assert!(app.world().get_entity(untracked).is_err());
        assert!(app.world().get_entity(unrelated).is_ok());
        let mut placed = app
            .world_mut()
            .query_filtered::<Entity, With<PlacedBlock>>();
        assert_eq!(placed.iter(app.world()).count(), 0);
        let mut stack = app.world_mut().resource_mut::<PlacedBlocks>();
        assert!(stack.records.is_empty());
        assert!(stack.redo.is_empty());
        assert!(stack.occupied.is_empty());
        assert!(stack.col_top.is_empty());
        assert!(stack.undo().is_none());
        assert!(!stack.redo(unrelated));
        assert_eq!(stack.revision, revision + 1);

        let blueprint = app.world().resource::<Blueprint>();
        assert_eq!(blueprint.def.id, "challenge-theme");
        assert_eq!(app.world().resource::<BlueprintLibrary>().current, 1);
        assert_eq!(blueprint.expected, [(IVec3::ZERO, "taiji".into())].into());
        assert_eq!(blueprint.cell_list, vec![IVec3::ZERO]);
        assert!(blueprint.active);
        assert!(!blueprint.completed);
        assert_eq!(blueprint.completion, 0.0);
        let challenge = app.world().resource::<Challenge>();
        assert_eq!(challenge.state, ChallengeState::Active);
        assert_eq!(challenge.time_left, 100.0);
        assert_eq!(challenge.quota_left, [("taiji".into(), 2)].into());
        assert_eq!(challenge.stars, 0);
        assert!(challenge.rewards.is_empty());
    }

    #[test]
    fn lifecycle_completion_awards_stars_once_and_freezes_terminal_timer() {
        for (elapsed, expected_stars) in [(49.0, 3), (50.0, 2), (70.0, 1)] {
            let mut app = lifecycle_app();
            lifecycle_step(&mut app, 0.0, true);
            complete_fixture_blueprint(&mut app);
            app.world_mut().resource_mut::<Blueprint>().active = false;
            lifecycle_step(&mut app, elapsed, false);
            assert!(app.world().resource::<Blueprint>().active);
            for _ in 0..3 {
                let challenge = app.world().resource::<Challenge>();
                assert_eq!(challenge.state, ChallengeState::Won);
                assert_eq!(challenge.stars, expected_stars);
                assert_eq!(challenge.rewards, vec!["baoding"]);
                assert_eq!(challenge.time_left, 100.0 - elapsed);
                lifecycle_step(&mut app, 200.0, false);
            }
        }
    }

    #[test]
    fn lifecycle_timeout_precedes_completion_and_cannot_reward_later() {
        for elapsed in [100.0, 101.0] {
            let mut app = lifecycle_app();
            lifecycle_step(&mut app, 0.0, true);
            complete_fixture_blueprint(&mut app);
            lifecycle_step(&mut app, elapsed, false);
            for _ in 0..3 {
                let challenge = app.world().resource::<Challenge>();
                assert_eq!(challenge.state, ChallengeState::Failed);
                assert_eq!(challenge.time_left, 0.0);
                assert_eq!(challenge.stars, 0);
                assert!(challenge.rewards.is_empty());
                lifecycle_step(&mut app, 1.0, false);
            }
        }
    }

    #[test]
    fn lifecycle_retry_after_win_or_timeout_starts_a_fresh_playable_run() {
        for fail_first in [false, true] {
            let mut app = lifecycle_app();
            lifecycle_step(&mut app, 0.0, true);
            complete_fixture_blueprint(&mut app);
            lifecycle_step(&mut app, if fail_first { 100.0 } else { 10.0 }, false);
            assert_eq!(
                app.world().resource::<Challenge>().state,
                if fail_first {
                    ChallengeState::Failed
                } else {
                    ChallengeState::Won
                }
            );
            app.world_mut().resource_mut::<Challenge>().consume("taiji");
            let previous = app.world().resource::<PlacedBlocks>().records[0].entity;
            lifecycle_step(&mut app, 0.0, true);
            assert!(app.world().get_entity(previous).is_err());
            assert!(app.world().resource::<PlacedBlocks>().records.is_empty());
            assert!(!app.world().resource::<Blueprint>().completed);
            let challenge = app.world().resource::<Challenge>();
            assert_eq!(challenge.state, ChallengeState::Active);
            assert_eq!(challenge.time_left, 100.0);
            assert_eq!(challenge.quota_left["taiji"], 2);
            assert_eq!(challenge.stars, 0);
            assert!(challenge.rewards.is_empty());
            complete_fixture_blueprint(&mut app);
            lifecycle_step(&mut app, 20.0, false);
            let challenge = app.world().resource::<Challenge>();
            assert_eq!(challenge.state, ChallengeState::Won);
            assert_eq!(challenge.stars, 3);
            assert_eq!(challenge.rewards, vec!["baoding"]);
        }
    }

    #[test]
    fn lifecycle_idle_does_not_tick_or_award_and_active_forces_blueprint_mode() {
        let mut app = lifecycle_app();
        lifecycle_step(&mut app, 200.0, false);
        let challenge = app.world().resource::<Challenge>();
        assert_eq!(challenge.state, ChallengeState::Idle);
        assert_eq!(challenge.time_left, 0.0);
        assert!(challenge.rewards.is_empty());
        assert!(!app.world().resource::<Blueprint>().active);
        lifecycle_step(&mut app, 0.0, true);
        app.world_mut().resource_mut::<Blueprint>().active = false;
        lifecycle_step(&mut app, 10.0, false);
        assert!(app.world().resource::<Blueprint>().active);
        let challenge = app.world().resource::<Challenge>();
        assert_eq!(challenge.state, ChallengeState::Active);
        assert_eq!(challenge.time_left, 90.0);
        assert!(challenge.rewards.is_empty());
    }
}

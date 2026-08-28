//! 结构稳定性（B-17）：avian3d 物理集成。
//!
//! - **X 键**：切换拆除模式——点击移除光标列最顶部的积木（支撑拆除）
//! - **G 键**：重力测试——把当前建筑转为动态刚体，沉降 3 秒后按
//!   「存活率」（位移 < 1 格的积木占比）评分并显示星级，随后自动复原原建筑
//!
//! 设计（ADR-002）：物理仅用于测试变体，正常放置/撤销保持确定性；
//! 测试结束按记录重建原建筑（网格数据不变，仅实体句柄重建）。

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::building::block_defs::BlockLibrary;
use crate::building::placement::{
    block_center, spawn_block_entity, BlockRenderAssets, PlacedBlock, PlacedBlocks,
};

/// 测试沉降时间（秒）
const SETTLE_SECS: f32 = 3.0;
/// 存活判定阈值（与原始位置距离 < 1 格）
const SURVIVAL_THRESHOLD: f32 = 1.0;

/// 拆除模式状态（X 键切换）
#[derive(Resource, Default)]
pub struct RemoveMode {
    pub active: bool,
}

/// 重力测试状态
#[derive(Resource, Default)]
pub struct StabilityTest {
    pub state: TestState,
    pub timer: f32,
    pub survival: f32,
    pub stars: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum TestState {
    #[default]
    Idle,
    Running,
    Done,
}

/// 已进入物理测试的积木标记
#[derive(Component)]
pub struct TestBlock;

pub struct StabilityPlugin;

impl Plugin for StabilityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RemoveMode::default())
            .insert_resource(StabilityTest::default())
            .add_plugins(PhysicsPlugins::default())
            .add_systems(Startup, setup_ground_collider)
            .add_systems(Update, (toggle_modes, gravity_test_tick).chain());
    }
}

/// 地面静态碰撞体（与主场景地面平面一致：130×130，顶面 y=0）。
fn setup_ground_collider(mut commands: Commands) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(65.0, 0.5, 65.0),
        Transform::from_xyz(0.0, -0.5, 25.0),
        Name::new("GroundCollider"),
    ));
}

/// X 切换拆除模式；G 启动/复原重力测试。
fn toggle_modes(
    keys: Res<ButtonInput<KeyCode>>,
    mut remove: ResMut<RemoveMode>,
    mut test: ResMut<StabilityTest>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    placed_query: Query<Entity, With<PlacedBlock>>,
) {
    if keys.just_pressed(KeyCode::KeyX) {
        remove.active = !remove.active;
        info!("🔧 拆除模式{}", if remove.active { "开启" } else { "关闭" });
    }

    if keys.just_pressed(KeyCode::KeyG) {
        match test.state {
            TestState::Idle => start_gravity_test(&mut test, &mut stack, &library, &mut commands),
            TestState::Running => {} // 测试中忽略
            TestState::Done => {
                // 复原原建筑
                for e in placed_query.iter() {
                    commands.entity(e).despawn();
                }
                rebuild_from_records(&mut commands, &mut stack, &library, &render);
                test.state = TestState::Idle;
                test.survival = 0.0;
                test.stars = 0;
                info!("🏗 已复原原建筑（重力测试结束）");
            }
        }
    }
}

/// 重力测试计时与评分。
fn gravity_test_tick(
    time: Res<Time>,
    mut test: ResMut<StabilityTest>,
    stack: Res<PlacedBlocks>,
    library: Res<BlockLibrary>,
    test_blocks: Query<(&Transform, Entity), With<TestBlock>>,
) {
    if test.state != TestState::Running {
        return;
    }
    test.timer -= time.delta_secs();
    if test.timer > 0.0 {
        return;
    }
    // 评分：存活率（原位置与当前位置距离 < 阈值）
    let mut originals = Vec::new();
    let mut currents = Vec::new();
    for r in &stack.records {
        let def = &library.defs[library.by_id[&r.def_id]];
        originals.push(block_center(r.anchor, def, r.rot));
        if let Ok((tf, _)) = test_blocks.get(r.entity) {
            currents.push(tf.translation);
        }
    }
    test.survival = survival_ratio(&originals, &currents, SURVIVAL_THRESHOLD);
    test.stars = stars_for_survival(test.survival);
    test.state = TestState::Done;
    info!(
        "🏗 重力测试完成：存活率 {:.0}% → {} 星（按 G 复原）",
        test.survival * 100.0,
        "★".repeat(test.stars as usize)
    );
}

/// 启动重力测试：把已放置积木转为动态刚体。
fn start_gravity_test(
    test: &mut StabilityTest,
    stack: &mut PlacedBlocks,
    library: &BlockLibrary,
    commands: &mut Commands,
) {
    if stack.records.is_empty() {
        info!("🏗 没有可测试的建筑（先放置积木）");
        return;
    }
    for r in &stack.records {
        let def = &library.defs[library.by_id[&r.def_id]];
        commands.entity(r.entity).insert((
            RigidBody::Dynamic,
            Collider::cuboid(
                def.size[0] as f32 * 0.5,
                def.size[1] as f32 * 0.5,
                def.size[2] as f32 * 0.5,
            ),
            TestBlock,
        ));
    }
    test.state = TestState::Running;
    test.timer = SETTLE_SECS;
    info!("🏗 重力测试开始：结构沉降 {SETTLE_SECS} 秒…");
}

/// 存活率评分（纯函数，可测试）。
pub fn stars_for_survival(survival: f32) -> u8 {
    if survival >= 0.95 {
        3
    } else if survival >= 0.8 {
        2
    } else if survival >= 0.5 {
        1
    } else {
        0
    }
}

/// 存活率（纯函数，可测试）：原位与当前位置距离 < 阈值的占比。
pub fn survival_ratio(originals: &[Vec3], currents: &[Vec3], threshold: f32) -> f32 {
    let total = currents.len().min(originals.len());
    if total == 0 {
        return 0.0;
    }
    let survived = currents
        .iter()
        .zip(originals.iter())
        .filter(|(c, o)| c.distance(**o) < threshold)
        .count();
    survived as f32 / total as f32
}

/// 复原：按记录重建积木实体（更新记录中的实体句柄）。
pub fn rebuild_from_records(
    commands: &mut Commands,
    stack: &mut PlacedBlocks,
    library: &BlockLibrary,
    render: &BlockRenderAssets,
) {
    for r in stack.records.iter_mut() {
        r.entity = spawn_block_entity(commands, library, render, &r.def_id, r.anchor, r.rot);
    }
    stack.revision += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survival_ratio_basic() {
        let originals = vec![Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0)];
        let currents = vec![Vec3::ZERO, Vec3::new(0.0, 0.5, 0.0)]; // 第二块掉落
        let s = survival_ratio(&originals, &currents, 1.0);
        assert!((s - 0.5).abs() < 1e-5, "存活率应为 0.5，实际 {s}");
    }

    #[test]
    fn stars_thresholds() {
        assert_eq!(stars_for_survival(0.97), 3);
        assert_eq!(stars_for_survival(0.85), 2);
        assert_eq!(stars_for_survival(0.6), 1);
        assert_eq!(stars_for_survival(0.3), 0);
    }

    #[test]
    fn empty_survival_is_zero() {
        assert_eq!(survival_ratio(&[], &[], 1.0), 0.0);
    }
}

//! 新手教程（B-12）：三步强制蓝图（台基 → 红柱 → 大梁），对应 PRD §6 新玩家路径。
//!
//! - 启动即激活，自动强制进入蓝图模式（幽灵蓝图由 reconcile 对账系统生成）
//! - 每步自动选中目标积木，HUD 显示引导文案
//! - 当前步骤目标格子全部正确放置后自动推进；全部完成或按 N 跳过
//! - 步骤完成判定为纯函数（step_complete），单元测试覆盖

use crate::ui::input::{shortcuts_allowed, InputOwnership};
use std::collections::HashMap;

use bevy::prelude::*;

use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::Blueprint;
use crate::building::placement::PlacedBlocks;

/// 教程步骤：目标积木 + 需覆盖的格子。
pub struct TutorialStep {
    pub block_id: String,
    pub label: String,
    pub label_en: String,
    pub cells: Vec<IVec3>,
}

#[derive(Resource)]
pub struct Tutorial {
    pub active: bool,
    pub step: usize,
    pub steps: Vec<TutorialStep>,
}

pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_tutorial())
            .add_systems(Update, tutorial_system)
            .add_systems(PostUpdate, tutorial_ghost_visibility);
    }
}

/// 加载教程步骤（依赖积木库尺寸生成目标格子）。
/// 教程对应五层收分黄鹤楼（B-05 官方比例）：基座 → 高柱 → 重檐大梁。
pub fn load_tutorial() -> Tutorial {
    let library = crate::building::block_defs::load_block_library();
    let datiji = &library.defs[library.by_id["datiji"]];
    let hongzhu4 = &library.defs[library.by_id["hongzhu4"]];
    let liangfang5 = &library.defs[library.by_id["liangfang5"]];

    // 第 1 步：大台基（7×2×7，锚点 (-3,0,-3)）
    let taiji_cells = crate::building::placement::footprint_cells(IVec3::new(-3, 0, -3), datiji, 0);

    // 第 2 步：一层高柱（四角 (±3,±3)，y 2..5，共 16 格）
    let mut zhu_cells = Vec::new();
    for sx in [-3, 3] {
        for sz in [-3, 3] {
            for y in 2..=5 {
                zhu_cells.push(IVec3::new(sx, y, sz));
            }
        }
    }
    debug_assert!(hongzhu4.size == [1, 4, 1]);

    // 第 3 步：一层重檐大梁（y=7，x -2..2 沿 z=-2 与 z=2，共 10 格）
    let mut liang_cells = Vec::new();
    for x in -2..=2 {
        liang_cells.push(IVec3::new(x, 7, -2));
        liang_cells.push(IVec3::new(x, 7, 2));
    }
    debug_assert!(liangfang5.size == [5, 1, 1]);

    Tutorial {
        active: true,
        step: 0,
        steps: vec![
            TutorialStep {
                block_id: "datiji".to_string(),
                label: "第 1 步（共 3 步）：放置大台基 —— 把绿色幽灵对准中央地面，左键点击放置".to_string(),
                label_en: "Step 1/3: Place the great stone platform — aim the green ghost at the ground center and left-click".to_string(),
                cells: taiji_cells,
            },
            TutorialStep {
                block_id: "hongzhu4".to_string(),
                label: "第 2 步（共 3 步）：放置高柱 —— 在大台基四角立起朱红高柱（已自动选中）".to_string(),
                label_en: "Step 2/3: Place tall vermilion columns at the four corners (auto-selected)".to_string(),
                cells: zhu_cells,
            },
            TutorialStep {
                block_id: "liangfang5".to_string(),
                label: "第 3 步（共 3 步）：架设重檐大梁 —— 在一层柱顶铺上五格大梁（已自动选中）".to_string(),
                label_en: "Step 3/3: Lay the five-cell eave beams across the first-tier columns (auto-selected)".to_string(),
                cells: liang_cells,
            },
        ],
    }
}

/// 当前步骤是否完成：目标格子全部被正确积木覆盖（纯函数，可测试）。
pub fn step_complete(placed: &HashMap<IVec3, String>, step: &TutorialStep) -> bool {
    step.cells
        .iter()
        .all(|c| placed.get(c).is_some_and(|id| id == &step.block_id))
}

/// 从放置记录构建 格子 → 积木ID 映射。
pub fn placed_map(stack: &PlacedBlocks) -> HashMap<IVec3, String> {
    stack
        .records
        .iter()
        .flat_map(|r| r.cells.iter().map(move |c| (*c, r.def_id.clone())))
        .collect()
}

/// 教程推进：强制蓝图模式、按步骤自动选中积木、完成检测、跳过（N 键）。
fn tutorial_system(
    ownership: Option<Res<InputOwnership>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut tutorial: ResMut<Tutorial>,
    mut blueprint: ResMut<Blueprint>,
    mut library: ResMut<BlockLibrary>,
    stack: Res<PlacedBlocks>,
) {
    if !tutorial.active {
        return;
    }

    // 强制进入蓝图模式（幽灵实体由 reconcile 系统生成）
    if !blueprint.active {
        blueprint.active = true;
    }

    // N 键跳过教程
    if shortcuts_allowed(&keys, ownership.as_deref()) && keys.just_pressed(KeyCode::KeyN) {
        tutorial.active = false;
        info!("🎓 教程已跳过，自由模式开放");
        return;
    }

    // Undo may remove an earlier prerequisite. Select the earliest unfinished
    // step before choosing its block; otherwise the player is locked out of repair.
    let placed = placed_map(&stack);
    if let Some(first_incomplete) = tutorial
        .steps
        .iter()
        .position(|step| !step_complete(&placed, step))
    {
        if first_incomplete < tutorial.step {
            tutorial.step = first_incomplete;
        }
    }

    // 当前步骤自动选中目标积木（含首次激活）
    if let Some(&idx) = library.by_id.get(&tutorial.steps[tutorial.step].block_id) {
        library.current = idx;
    }

    // 完成检测
    let step = &tutorial.steps[tutorial.step];
    if !step_complete(&placed, step) {
        return;
    }

    // 推进
    tutorial.step += 1;
    if tutorial.step >= tutorial.steps.len() {
        tutorial.active = false;
        info!("🎓 新手教程完成！已解锁完整蓝图与自由模式（M 键切换）");
    } else {
        info!("🎓 {}", tutorial.steps[tutorial.step].label);
    }
}

/// Keep the construction target legible; undo restores its unoccupied guide cells.
fn tutorial_ghost_visibility(
    tutorial: Res<Tutorial>,
    stack: Res<PlacedBlocks>,
    mut ghosts: Query<
        (&Transform, &mut Visibility),
        With<crate::building::blueprint::BlueprintGhost>,
    >,
) {
    let current = tutorial
        .active
        .then(|| tutorial.steps.get(tutorial.step))
        .flatten();
    for (transform, mut visibility) in &mut ghosts {
        let cell = (transform.translation - Vec3::Y * 0.5).round().as_ivec3();
        let visible = !stack.occupied.contains(&cell)
            && current.is_none_or(|step| step.cells.contains(&cell));
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::blueprint::load_blueprint;

    #[test]
    fn tutorial_blueprint_hides_future_and_occupied_cells_then_restores_overview() {
        use crate::building::blueprint::BlueprintGhost;
        let mut app = App::new();
        app.insert_resource(load_tutorial())
            .init_resource::<PlacedBlocks>()
            .add_systems(Update, tutorial_ghost_visibility);
        let current = IVec3::new(-3, 0, -3);
        let future = IVec3::new(-3, 2, -3);
        let spawn = |app: &mut App, cell: IVec3| {
            app.world_mut()
                .spawn((
                    BlueprintGhost,
                    Transform::from_translation(cell.as_vec3() + Vec3::Y * 0.5),
                    Visibility::Inherited,
                ))
                .id()
        };
        let a = spawn(&mut app, current);
        let b = spawn(&mut app, future);
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(a).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            *app.world().get::<Visibility>(b).unwrap(),
            Visibility::Hidden
        );
        app.world_mut()
            .resource_mut::<PlacedBlocks>()
            .occupied
            .insert(current);
        app.world_mut().resource_mut::<Tutorial>().active = false;
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(a).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            *app.world().get::<Visibility>(b).unwrap(),
            Visibility::Inherited
        );
        app.world_mut()
            .resource_mut::<PlacedBlocks>()
            .occupied
            .clear();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(a).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn undoing_platform_rewinds_active_tutorial_and_selection() {
        let mut app = App::new();
        let mut tutorial = load_tutorial();
        tutorial.step = 1;
        app.insert_resource(tutorial)
            .insert_resource(crate::building::blueprint::load_blueprint_library().0)
            .insert_resource(crate::building::block_defs::load_block_library())
            .init_resource::<PlacedBlocks>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, tutorial_system);
        app.update();
        assert_eq!(app.world().resource::<Tutorial>().step, 0);
        assert_eq!(
            app.world().resource::<BlockLibrary>().current_def().id,
            "datiji"
        );
    }

    #[test]
    fn empty_placement_not_complete() {
        let tut = load_tutorial();
        let placed = HashMap::new();
        assert!(!step_complete(&placed, &tut.steps[0]));
    }

    #[test]
    fn partial_placement_not_complete() {
        let tut = load_tutorial();
        // 只放 1 格台基 → 未完成
        let mut placed = HashMap::new();
        placed.insert(IVec3::new(0, 0, 0), "taiji".to_string());
        assert!(!step_complete(&placed, &tut.steps[0]));
    }

    #[test]
    fn full_taiji_completes_step_one() {
        let tut = load_tutorial();
        let placed: HashMap<IVec3, String> = tut.steps[0]
            .cells
            .iter()
            .map(|c| (*c, tut.steps[0].block_id.clone()))
            .collect();
        assert!(step_complete(&placed, &tut.steps[0]));
    }

    #[test]
    fn wrong_block_does_not_complete() {
        let tut = load_tutorial();
        // 用 hongzhu 填台基格 → 不完成
        let placed: HashMap<IVec3, String> = tut.steps[0]
            .cells
            .iter()
            .map(|c| (*c, "hongzhu".to_string()))
            .collect();
        assert!(!step_complete(&placed, &tut.steps[0]));
    }

    #[test]
    fn tutorial_has_three_steps_with_valid_block_ids() {
        let tut = load_tutorial();
        let lib = crate::building::block_defs::load_block_library();
        assert_eq!(tut.steps.len(), 3);
        for step in &tut.steps {
            assert!(
                lib.by_id.contains_key(&step.block_id),
                "教程引用未知积木 {}",
                step.block_id
            );
            assert!(!step.cells.is_empty());
        }
    }

    #[test]
    fn blueprint_reference_integrity() {
        // 教程格子应属于蓝图期望格（与 B-08 占位蓝图一致）
        let tut = load_tutorial();
        let bp = load_blueprint();
        for step in &tut.steps {
            for cell in &step.cells {
                assert!(
                    bp.expected.contains_key(cell),
                    "教程格子 {cell:?} 不在蓝图中"
                );
                assert_eq!(
                    bp.expected[cell], step.block_id,
                    "教程格子 {cell:?} 期望积木不一致"
                );
            }
        }
    }
}

//! 蓝图模式（B-08）：半透明幽灵蓝图 + 严格吸附 + 稀疏格子匹配。
//!
//! 数据结构与算法见 ADR-005：
//! - 蓝图表：`HashMap<IVec3, BlockId>`（期望格子集合）
//! - 匹配：放置块 footprint 与蓝图表同构比较
//! - 完成度：层级加权 `Σ(权重 × 层级匹配率)`，空层级按 1.0 计
//! - 权重：台基 .25 / 柱 .25 / 梁 .2 / 楼板 .1 / 屋顶 .15 / 装饰 .05
//!
//! 匹配算法为纯函数，单元测试见文件底部（对齐/偏移/多余/错块四情形）。

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use serde::Deserialize;

use super::block_defs::BlockLibrary;

/// 蓝图定义（resources/blueprints/*.ron）。
// id 供 Phase 2 多蓝图选择（主题包 B-24）使用，当前未消费，故 allow(dead_code)。
#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct BlueprintDef {
    pub id: String,
    pub name: String,
    /// (x, y, z, block_id)：y 为层高（格），x/z 为平面坐标
    pub cells: Vec<(i32, i32, i32, String)>,
}

/// 蓝图运行态资源。
#[derive(Resource)]
pub struct Blueprint {
    pub def: BlueprintDef,
    /// 期望格子集合：cell → block id（ADR-005）
    pub expected: HashMap<IVec3, String>,
    /// 有序格子列表（用于生成幽灵蓝图实体）
    pub cell_list: Vec<IVec3>,
    /// 蓝图模式开关（M 键切换）
    pub active: bool,
    /// 当前完成度（0..1，层级加权）
    pub completion: f32,
    /// 是否已触发完成事件（防止重复提示）
    pub completed: bool,
}

/// 幽灵蓝图实体标记
#[derive(Component)]
pub struct BlueprintGhost;

/// 从 resources/blueprints/*.ron 加载（当前取第一个；多蓝图选择 Phase 2）。
pub fn load_blueprint() -> Blueprint {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/blueprints");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("无法读取蓝图目录 {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    let Some(path) = files.first() else {
        panic!("resources/blueprints/ 下未找到任何蓝图 (.ron)");
    };
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("读取蓝图失败 {}: {e}", path.display()));
    let def: BlueprintDef = ron::from_str(&text)
        .unwrap_or_else(|e| panic!("解析蓝图失败 {}: {e}", path.display()));

    let expected = def
        .cells
        .iter()
        .map(|(x, y, z, id)| (IVec3::new(*x, *y, *z), id.clone()))
        .collect();
    let cell_list = def
        .cells
        .iter()
        .map(|(x, y, z, _)| IVec3::new(*x, *y, *z))
        .collect();
    Blueprint {
        def,
        expected,
        cell_list,
        active: false,
        completion: 0.0,
        completed: false,
    }
}

/// 放置块 footprint 是否完全匹配蓝图期望（每格 id 一致且均在蓝图内）。
pub fn footprint_matches(expected: &HashMap<IVec3, String>, cells: &[IVec3], def_id: &str) -> bool {
    cells.iter().all(|c| expected.get(c).is_some_and(|id| id == def_id))
}

/// ADR-005 层级权重。
pub fn layer_weight(layer: &str) -> f32 {
    match layer {
        "台基" => 0.25,
        "柱" => 0.25,
        "梁" => 0.20,
        "楼板" => 0.10,
        "屋顶" => 0.15,
        _ => 0.05, // 装饰 / 未知层级
    }
}

/// 完成度（ADR-005）：Σ(层级权重 × 层级匹配率)。
/// 只统计蓝图期望格子；蓝图外的多余积木不影响完成度。
pub fn compute_completion(
    expected: &HashMap<IVec3, String>,
    placed: &HashMap<IVec3, String>,
    library: &BlockLibrary,
) -> f32 {
    let mut layer_total: HashMap<&str, usize> = HashMap::new();
    let mut layer_matched: HashMap<&str, usize> = HashMap::new();
    for (cell, id) in expected {
        let layer = layer_of(library, id);
        *layer_total.entry(layer).or_default() += 1;
        if placed.get(cell).is_some_and(|pid| pid == id) {
            *layer_matched.entry(layer).or_default() += 1;
        }
    }
    let mut sum = 0.0;
    let mut weight_sum = 0.0;
    for (layer, total) in &layer_total {
        let w = layer_weight(layer);
        weight_sum += w;
        let matched = layer_matched.get(layer).copied().unwrap_or(0);
        sum += w * (matched as f32 / *total as f32);
    }
    if weight_sum == 0.0 {
        0.0
    } else {
        sum / weight_sum
    }
}

/// 由积木 ID 查结构层级（未知 ID 按装饰计）。
fn layer_of<'a>(library: &'a BlockLibrary, id: &str) -> &'a str {
    library
        .by_id
        .get(id)
        .map(|i| library.defs[*i].layer.as_str())
        .unwrap_or("装饰")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::block_defs::load_block_library;

    fn setup() -> (Blueprint, BlockLibrary) {
        (load_blueprint(), load_block_library())
    }

    #[test]
    fn empty_placement_gives_zero_completion() {
        let (bp, lib) = setup();
        let placed = HashMap::new();
        assert_eq!(compute_completion(&bp.expected, &placed, &lib), 0.0);
    }

    #[test]
    fn exact_match_completes() {
        let (bp, lib) = setup();
        let placed: HashMap<IVec3, String> = bp.expected.clone();
        let c = compute_completion(&bp.expected, &placed, &lib);
        assert!((c - 1.0).abs() < 1e-5, "完成度应 = 1.0，实际 {c}");
    }

    #[test]
    fn offset_placement_lowers_completion() {
        let (bp, lib) = setup();
        // 整体偏移 +1（x 轴）：自相似结构（同色大块）仍会损失显著匹配
        let placed: HashMap<IVec3, String> = bp
            .expected
            .iter()
            .map(|(c, id)| (*c + IVec3::new(1, 0, 0), id.clone()))
            .collect();
        let c = compute_completion(&bp.expected, &placed, &lib);
        assert!(
            (0.3..1.0).contains(&c),
            "偏移 +1 应显著降低完成度（非满分），实际 {c}"
        );
    }

    #[test]
    fn large_offset_nearly_zero_completion() {
        let (bp, lib) = setup();
        // 整体偏移 +5（超出塔身宽度）：几乎全部错位
        let placed: HashMap<IVec3, String> = bp
            .expected
            .iter()
            .map(|(c, id)| (*c + IVec3::new(5, 0, 0), id.clone()))
            .collect();
        let c = compute_completion(&bp.expected, &placed, &lib);
        assert!(c < 0.1, "大幅偏移应接近 0，实际 {c}");
    }

    #[test]
    fn extra_blocks_do_not_affect_completion() {
        let (bp, lib) = setup();
        let mut placed: HashMap<IVec3, String> = bp.expected.clone();
        // 蓝图外多余积木（自由模式遗留）不影响完成度
        placed.insert(IVec3::new(99, 0, 99), "hongzhu".to_string());
        let c = compute_completion(&bp.expected, &placed, &lib);
        assert!((c - 1.0).abs() < 1e-5, "多余积木不应影响完成度，实际 {c}");
    }

    #[test]
    fn footprint_matches_rejects_wrong_block() {
        let (bp, _lib) = setup();
        let cell = *bp.cell_list.first().unwrap();
        let expected_id = &bp.expected[&cell];
        assert!(footprint_matches(&bp.expected, &[cell], expected_id));
        assert!(!footprint_matches(&bp.expected, &[cell], "hongzhu"));
    }
}

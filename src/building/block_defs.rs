//! 积木定义（数据驱动，B-06）。
//!
//! 定义全部位于 `resources/blocks/*.ron`（ID、尺寸、材质色、吸附规则、文化描述），
//! 对应 PRD §4「数据驱动」规约与 ADR-005 层级权重体系。
//! 运行期扫描目录加载为 [`BlockLibrary`] 资源；新增积木只需添加一个 RON 文件。
//!
//! 待办（见 BACKLOG B-06）：开发期热重载（bevy asset watching）留待 Phase 1 完整版。

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use serde::Deserialize;

/// 单个积木定义（与 resources/blocks/*.ron 一一对应）。
// category/snap/description 供 Phase 1 图鉴（B-18）、智能磁吸（B-06 完整版）、
// 知识卡片使用，当前阶段未消费，故 allow(dead_code)。
#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct BlockDef {
    /// 稳定唯一 ID（发布后不可复用，见 ADR-006 存档兼容策略）
    pub id: String,
    /// 中文名
    pub name: String,
    /// 分类（对应 PRD §3.1 积木分类）
    pub category: BlockCategory,
    /// 结构层级（对应 ADR-005 完成度加权）：台基 / 柱 / 梁 / 楼板 / 屋顶 / 装饰
    pub layer: String,
    /// 积木面板排序
    pub order: u32,
    /// 占用格子 (w, h, d)，h 为层高（格数）
    pub size: [u32; 3],
    /// 基色 RGBA（0..1，sRGB 语义）
    pub color: [f32; 4],
    /// 吸附模式：网格（默认）/ 磁吸（Phase 1 实现智能磁吸）
    pub snap: SnapMode,
    /// 文化描述（图鉴/知识卡片素材，对应 PRD §3.4）
    pub description: String,
}

/// 积木分类（PRD §3.1）。
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockCategory {
    /// 基础：台基、柱础
    Base,
    /// 结构：柱、梁枋、斗拱、楼板、栏杆
    Structure,
    /// 屋顶：瓦片、飞檐、宝顶、脊兽
    Roof,
    /// 装饰：窗棂、匾额
    Decoration,
    /// 特殊：可活动部件
    Special,
}

/// 吸附模式。
#[derive(Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapMode {
    #[default]
    Grid,
    Magnetic,
}

/// 积木库资源：全部定义 + 索引 + 当前选中。
#[derive(Resource)]
pub struct BlockLibrary {
    pub defs: Vec<BlockDef>,
    pub by_id: HashMap<String, usize>,
    /// 当前选中的积木索引（HUD/快捷键/面板选择）
    pub current: usize,
    /// 当前选中积木的旋转（0..=3，×90°，R 键切换，PRD §3.1 旋转 90° 限制）
    pub rotation: u8,
}

impl BlockLibrary {
    pub fn current_def(&self) -> &BlockDef {
        &self.defs[self.current]
    }
}

/// 从 `resources/blocks/*.ron` 加载积木库（启动期 fail-fast，避免静默缺资产）。
pub fn load_block_library() -> BlockLibrary {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/blocks");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("无法读取积木目录 {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();

    let mut defs: Vec<BlockDef> = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("读取积木定义失败 {}: {e}", path.display()));
        let def: BlockDef = ron::from_str(&text)
            .unwrap_or_else(|e| panic!("解析积木定义失败 {}: {e}", path.display()));
        defs.push(def);
    }
    if defs.is_empty() {
        panic!("resources/blocks/ 下未找到任何积木定义 (.ron)");
    }
    defs.sort_by_key(|d| d.order);

    let by_id = defs
        .iter()
        .enumerate()
        .map(|(i, d)| (d.id.clone(), i))
        .collect();
    BlockLibrary {
        defs,
        by_id,
        current: 0,
        rotation: 0,
    }
}

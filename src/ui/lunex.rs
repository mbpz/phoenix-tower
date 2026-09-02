//! Bevy-Lunex UI（A1：基础集成与最小界面，docs/UI_LUNEX_MIGRATION.md）
//!
//! 参考 Bevypunk（IDEDARY/Bevypunk，Cyberpunk UI 复刻）的 UI 架构：
//! - 独立 `Camera2d`（透明清屏、order 高于 3D 主相机）叠加在场景之上；
//! - 该相机挂 `UiSourceCamera::<0>`，UI 根实体挂 `UiLayoutRoot::new_2d()` +
//!   `UiFetchFromCamera::<0>` 自动同步视口尺寸；
//! - 文本走 Bevy 原生 `Text2d` + `UiTextSize`（lunex 按父节点比例缩放）。
//!
//! 交互（A3）：lunex 0.7 基于 bevy_picking（`Pointer<Click/Over/Out>` 观察者、
//! `Pickable`）。本阶段先验证渲染，交互在 A3 落地。

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy_lunex::prelude::*;
use bevy_lunex::UiSelected;
use bevy::picking::pointer::PointerId;
use bevy_rich_text3d::{LoadFonts, Text3d, Text3dStyling};

use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::{
    select_blueprint, Blueprint, BlueprintGhost, BlueprintLibrary,
};
use crate::building::collection::{achievement_defs, Collection, KnowledgeHints};
use crate::building::challenge::{
    select_challenge, start_challenge, Challenge, ChallengeLibrary, ChallengeState,
};
use crate::building::placement::{BlockRenderAssets, BlueprintAlpha, PlacedBlock, PlacedBlocks, PlaqueText};
use crate::save::{
    export_json, import_latest, import_save, load_save_from_path, load_slot, save_file_list,
    save_slot, saves_dir, share_export,
};
use std::path::Path;

pub struct LunexUiPlugin;

impl Plugin for LunexUiPlugin {
    fn build(&self, app: &mut App) {
        // 让 bevy_rich_text3d 的字库包含我们的 CJK 子集字体（A2 验证 + C1 匾额用）。
        // Text3dPlugin 在插件组 cleanup 时读取本资源；init_resource 不覆盖已存在的值，
        // 因此这里先填充再 add_plugins。
        app.init_resource::<LoadFonts>();
        let subset = format!("{}/assets/fonts/NotoSansSC-subset.otf", env!("CARGO_MANIFEST_DIR"));
        app.world_mut()
            .resource_mut::<LoadFonts>()
            .font_paths
            .push(subset);
        app.init_resource::<LunexTheme>()
            .init_resource::<PaletteScroll>()
            .init_resource::<ThemeMenuOpen>()
            .init_resource::<ChallengeMenuOpen>()
            .init_resource::<LunexTab>()
            .init_resource::<PathInput>()
            .init_resource::<CodexScroll>()
            .init_resource::<CodexSelected>()
            .init_resource::<OpacityDragging>()
            .add_plugins(UiLunexPlugins)
            .add_systems(
                Startup,
                (spawn_ui_camera, spawn_hud_root, spawn_text3d_probe),
            )
            .add_systems(
                Update,
                (
                    palette_scroll_system,
                    palette_sync_selection,
                    b2_progress_sync,
                    b2_theme_menu_sync,
                    b2_theme_name_sync,
                    b3_challenge_sync,
                    b3_challenge_menu_sync,
                    b3_challenge_name_sync,
                ),
            )
            .add_systems(
                Update,
                (
                    lunex_tab_sync,
                    save_list_system,
                    path_input_system,
                    codex_scroll_system,
                    b5_codex_detail_sync,
                    b5_codex_status_sync,
                    b6_achievement_sync,
                    b7_knowledge_sync,
                    b8_opacity_sync,
                    b8_opacity_apply,
                    ui_probe_diagnostic,
                    ui_probe_tabs,
                ),
            );
    }
}

// ======================================================================
// A4：UI 主题与层级
// ----------------------------------------------------------------------
// 层级约定（lunex 用 UiDepth 表达，默认 0 即互不遮挡时无需显式设置）：
//   - HUD 层：顶部标题横幅 + 未来世界空间提示（深色半透明底）
//   - 面板层：左侧积木面板（B1+）
// 后续 B 阶段所有颜色统一走 LunexTheme，杜绝魔法数字。
// ======================================================================

/// Lunex UI 主题：深蓝灰底 + 金色点缀（与 3D 场景灯笼/匾额金色呼应）
#[derive(Resource)]
pub struct LunexTheme {
    pub banner_bg: Color,
    pub panel_bg: Color,
    pub row_base: Color,
    pub row_hover: Color,
    pub row_selected: Color,
    pub text_main: Color,
    pub text_dim: Color,
    pub accent: Color,
}

impl Default for LunexTheme {
    fn default() -> Self {
        Self {
            banner_bg: Color::srgba(0.09, 0.13, 0.19, 0.82),
            panel_bg: Color::srgba(0.055, 0.085, 0.14, 0.92),
            row_base: Color::srgba(0.09, 0.13, 0.20, 0.90),
            row_hover: Color::srgba(0.16, 0.22, 0.32, 0.95),
            row_selected: Color::srgba(0.44, 0.35, 0.16, 0.95),
            text_main: Color::srgb(0.94, 0.91, 0.84),
            text_dim: Color::srgb(0.58, 0.55, 0.48),
            accent: Color::srgb(0.95, 0.85, 0.40),
        }
    }
}

// ======================================================================
// B1：积木面板（lunex 版）
// ----------------------------------------------------------------------
// 默认不生成（egui 面板并行保留）。`PHOENIX_LUNEX_PALETTE=1` 启用，
// 与 egui 面板暂时重叠属预期（B 阶段完成前两者共存）。
// 滚动：行按「行单位」偏移定位，滚轮滚动（仅指针悬停于行上时），
// 窗口外的行置 Visibility::Hidden（同时自动退出 picking）。
// ======================================================================

/// 面板内可见行数（窗口高度按此均分）
const PALETTE_VISIBLE: usize = 10;
/// 单行高度 = 100% / 可见行数
const ROW_H_PCT: f32 = 100.0 / PALETTE_VISIBLE as f32;

/// 积木面板行标记（存积木索引）
#[derive(Component)]
pub struct PaletteRow(pub usize);

/// 面板滚动偏移（行单位，0 = 顶部）
#[derive(Resource, Default)]
pub struct PaletteScroll(pub f32);

/// 生成积木面板（B1）：标题 + 滚轮滚动列表 + 点击选中 + 选中高亮。
/// 挂在 Lunex HUD Root 之下（由 spawn_hud_root 调用，env 门控）。
fn spawn_palette_nodes(
    ui: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    library: &BlockLibrary,
    blueprint: &Blueprint,
    blueprint_library: &BlueprintLibrary,
    challenge: &Challenge,
    challenge_library: &ChallengeLibrary,
    collection: &Collection,
    materials: &mut Assets<ColorMaterial>,
    theme: &LunexTheme,
) {
    let font = FontSource::Handle(asset_server.load("fonts/NotoSansSC-subset.otf"));
    let panel_material = materials.add(ColorMaterial::from(theme.panel_bg));

    // 左侧面板容器
    ui.spawn((
        Name::new("Block Palette"),
        UiLayout::window()
            .pos((Rl(1.4), Rh(50.0)))
            .size((Rl(21.0), Rh(96.0)))
            .anchor(Anchor::CENTER_LEFT)
            .pack(),
        UiMeshPlane2d,
        MeshMaterial2d(panel_material.clone()),
        Pickable::default(),
    ))
    .with_children(|panel| {
        // Tab 栏：积木 / 图鉴 / 成就 / 存档
        for (i, (tab, label)) in [
            (LunexTabId::Blocks, "积木"),
            (LunexTabId::Codex, "图鉴"),
            (LunexTabId::Achievements, "成就"),
            (LunexTabId::Saves, "存档"),
        ]
        .iter()
        .enumerate()
        {
            let x = 12.75 + i as f32 * 25.0;
            let tab_material = materials.add(ColorMaterial::from(theme.row_base));
            panel.spawn((
                Name::new(format!("tab_{:?}", tab)),
                UiLayout::window()
                    .pos((Rl(x), Rh(3.0)))
                    .size((Rl(23.5), Rh(6.5)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiHover::id(), theme.row_hover),
                    (UiSelected::id(), theme.row_selected),
                ]),
                UiHover::new().instant(true),
                UiSelected(0.0),
                UiMeshPlane2d,
                MeshMaterial2d(tab_material),
                TabButton(*tab),
            ))
            .observe(hover_set::<Pointer<Over>, true>)
            .observe(hover_set::<Pointer<Out>, false>)
            .observe(tab_button_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("tab_label"),
                    Text2d::new(label.to_string()),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(50.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window().full().pack(),
                    Pickable::IGNORE,
                ));
            });
        }

        // 标题
        panel.spawn((
            Name::new("Palette Title"),
            Text2d::new("积木"),
            TextFont {
                font: font.clone(),
                font_size: FontSize::Px(26.0),
                ..default()
            },
            UiTextSize::from(Rh(4.5)),
            UiColor::new(vec![(UiBase::id(), theme.accent)]),
            UiLayout::window()
                .pos((Rl(50.0), Rh(10.5)))
                .anchor(Anchor::TOP_CENTER)
                .pack(),
            Pickable::IGNORE,
            TabBlocksRoot,
        ));

        // B2 蓝图模式区：完成度进度条 + 主题下拉
        panel
            .spawn((
                Name::new("Blueprint Section"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(14.0)))
                    .size((Rl(94.0), Rh(21.0)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                Pickable::IGNORE,
                TabBlocksRoot,
            ))
            .with_children(|sec| {
                // 「蓝图」标签
                sec.spawn((
                    Name::new("Blueprint Label"),
                    Text2d::new("蓝图"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(26.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(5.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                ));
                // 进度条底 + 填充
                let bar_material =
                    materials.add(ColorMaterial::from(Color::srgba(0.02, 0.03, 0.05, 0.9)));
                let fill_material = materials.add(ColorMaterial::from(theme.accent));
                sec.spawn((
                    Name::new("Progress Bar"),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(42.0)))
                        .size((Rl(72.0), Rh(24.0)))
                        .anchor(Anchor::CENTER_LEFT)
                        .pack(),
                    UiMeshPlane2d,
                    MeshMaterial2d(bar_material.clone()),
                    Pickable::IGNORE,
                ))
                .with_children(|bar| {
                    let pct = (blueprint.completion.clamp(0.0, 1.0) * 100.0).round();
                    bar.spawn((
                        Name::new("Progress Fill"),
                        UiLayout::window()
                            .pos((Rl(1.5), Rh(50.0)))
                            .size((Rl(pct), Rl(82.0)))
                            .anchor(Anchor::CENTER_LEFT)
                            .pack(),
                        UiMeshPlane2d,
                        MeshMaterial2d(fill_material.clone()),
                        Pickable::IGNORE,
                        ProgressFill,
                    ));
                });
                // 完成度文本
                sec.spawn((
                    Name::new("Progress Text"),
                    Text2d::new(format!("{:.0}%", blueprint.completion.clamp(0.0, 1.0) * 100.0)),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(20.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window()
                        .pos((Rl(79.0), Rh(42.0)))
                        .anchor(Anchor::CENTER_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                    ProgressText,
                ));
                // 主题按钮（全宽；点击开合下拉）
                let btn_material = materials.add(ColorMaterial::from(theme.row_base));
                sec.spawn((
                    Name::new("Theme Button"),
                    UiLayout::window()
                        .pos((Rl(50.0), Rh(82.0)))
                        .size((Rl(92.0), Rh(30.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(btn_material),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(theme_button_click)
                .with_children(|btn| {
                    btn.spawn((
                        Name::new("Theme Button Text"),
                        Text2d::new(blueprint_library.current_def().name.clone()),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(50.0)),
                        UiColor::new(vec![(UiBase::id(), theme.accent)]),
                        UiLayout::window().full().pack(),
                        Pickable::IGNORE,
                        ThemeButtonText,
                    ));
                });
                // 下拉列表（默认隐藏；按钮开合）。Ab 像素尺寸保证可点击高度；
                // 行 y 超出 section 底部即覆盖到滚动列表上（lunex 不裁剪）。
                for (i, def) in blueprint_library.defs.iter().enumerate() {
                    let y = 124.0 + i as f32 * 31.0;
                    let row_material = materials.add(ColorMaterial::from(theme.row_base));
                    sec.spawn((
                        Name::new(format!("theme_row_{}", def.id)),
                        UiLayout::window()
                            .pos((Rl(50.0), Ab(y)))
                            .size((Rl(92.0), Ab(27.0)))
                            .anchor(Anchor::TOP_CENTER)
                            .pack(),
                        UiColor::new(vec![
                            (UiBase::id(), theme.row_base),
                            (UiHover::id(), theme.row_hover),
                        ]),
                        UiHover::new().instant(true),
                        UiMeshPlane2d,
                        MeshMaterial2d(row_material),
                        Visibility::Hidden,
                        ThemeMenuRow(i),
                    ))
                    .observe(hover_set::<Pointer<Over>, true>)
                    .observe(hover_set::<Pointer<Out>, false>)
                    .observe(theme_menu_row_click)
                    .with_children(|row| {
                        row.spawn((
                            Name::new("theme_row_text"),
                            Text2d::new(def.name.clone()),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            UiTextSize::from(Rh(52.0)),
                            UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                            UiLayout::window()
                                .pos((Rl(8.0), Rl(50.0)))
                                .anchor(Anchor::CENTER_LEFT)
                                .pack(),
                            Pickable::IGNORE,
                        ));
                    });
                }
                // B8 蓝图透明度滑杆
                sec.spawn((
                    Name::new("Opacity Label"),
                    Text2d::new("透明度"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(3.6)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(91.0)))
                        .anchor(Anchor::CENTER_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                ));
                let track_mat = materials.add(ColorMaterial::from(Color::srgba(0.02, 0.03, 0.05, 0.9)));
                let fill_mat = materials.add(ColorMaterial::from(theme.accent));
                let knob_mat = materials.add(ColorMaterial::from(theme.text_main));
                let init_frac = ((BlueprintAlpha::default().value - 0.1) / 0.7).clamp(0.0, 1.0);
                sec.spawn((
                    Name::new("Opacity Slider Track"),
                    UiLayout::window()
                        .pos((Rl(42.0), Rh(91.0)))
                        .size((Rl(52.0), Rh(5.0)))
                        .anchor(Anchor::CENTER_LEFT)
                        .pack(),
                    UiMeshPlane2d,
                    MeshMaterial2d(track_mat),
                    Pickable::default(),
                    OpacitySlider,
                ))
                .observe(opacity_press)
                .observe(opacity_move)
                .observe(opacity_release)
                .with_children(|track| {
                    track.spawn((
                        Name::new("Opacity Fill"),
                        UiLayout::window()
                            .pos((Rl(1.0), Rh(50.0)))
                            .size((Rl(init_frac * 98.0), Rl(70.0)))
                            .anchor(Anchor::CENTER_LEFT)
                            .pack(),
                        UiMeshPlane2d,
                        MeshMaterial2d(fill_mat.clone()),
                        Pickable::IGNORE,
                        OpacityFill,
                    ));
                    track.spawn((
                        Name::new("Opacity Knob"),
                        UiLayout::window()
                            .pos((Rl(init_frac * 100.0), Rl(50.0)))
                            .size((Rl(3.0), Rh(150.0)))
                            .anchor(Anchor::CENTER)
                            .pack(),
                        UiMeshPlane2d,
                        MeshMaterial2d(knob_mat),
                        Pickable::IGNORE,
                        OpacityKnob,
                    ));
                });
            });

        // B3 挑战区：选择下拉 + 状态/倒计时/材料 + 开始/重试按钮
        panel
            .spawn((
                Name::new("Challenge Section"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(37.0)))
                    .size((Rl(94.0), Rh(17.0)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                Pickable::IGNORE,
                TabBlocksRoot,
            ))
            .with_children(|sec| {
                // 挑战选择下拉按钮
                let chal_btn_material = materials.add(ColorMaterial::from(theme.row_base));
                sec.spawn((
                    Name::new("Challenge Button"),
                    UiLayout::window()
                        .pos((Rl(2.0), Rh(26.0)))
                        .size((Rl(62.0), Rh(40.0)))
                        .anchor(Anchor::CENTER_LEFT)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(chal_btn_material),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(challenge_button_click)
                .with_children(|btn| {
                    btn.spawn((
                        Name::new("Challenge Button Text"),
                        Text2d::new(challenge.def.name.clone()),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(50.0)),
                        UiColor::new(vec![(UiBase::id(), theme.accent)]),
                        UiLayout::window()
                            .pos((Rl(50.0), Rl(50.0)))
                            .anchor(Anchor::CENTER)
                            .pack(),
                        Pickable::IGNORE,
                        ChallengeNameText,
                    ));
                });
                // 开始/重试按钮
                let start_material = materials.add(ColorMaterial::from(theme.row_base));
                sec.spawn((
                    Name::new("Challenge Start"),
                    UiLayout::window()
                        .pos((Rl(67.0), Rh(26.0)))
                        .size((Rl(31.0), Rh(40.0)))
                        .anchor(Anchor::CENTER_LEFT)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(start_material),
                    Pickable::default(),
                    ChallengeStartButton,
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(challenge_start_click)
                .with_children(|btn| {
                    btn.spawn((
                        Name::new("Challenge Start Text"),
                        Text2d::new("开始"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(50.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window()
                            .pos((Rl(50.0), Rl(50.0)))
                            .anchor(Anchor::CENTER)
                            .pack(),
                        Pickable::IGNORE,
                        ChallengeButtonText,
                    ));
                });
                // 状态行
                sec.spawn((
                    Name::new("Challenge Status"),
                    Text2d::new("🏆 未开始"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(18.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(76.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                    ChallengeStatusText,
                ));
                // 挑战下拉列表（默认隐藏）
                for (i, def) in challenge_library.defs.iter().enumerate() {
                    let y = 88.0 + i as f32 * 31.0;
                    let row_material = materials.add(ColorMaterial::from(theme.row_base));
                    sec.spawn((
                        Name::new(format!("challenge_row_{}", def.id)),
                        UiLayout::window()
                            .pos((Rl(2.0), Ab(y)))
                            .size((Rl(62.0), Ab(27.0)))
                            .anchor(Anchor::TOP_LEFT)
                            .pack(),
                        UiColor::new(vec![
                            (UiBase::id(), theme.row_base),
                            (UiHover::id(), theme.row_hover),
                        ]),
                        UiHover::new().instant(true),
                        UiMeshPlane2d,
                        MeshMaterial2d(row_material),
                        Visibility::Hidden,
                        ChallengeMenuRow(i),
                    ))
                    .observe(hover_set::<Pointer<Over>, true>)
                    .observe(hover_set::<Pointer<Out>, false>)
                    .observe(challenge_menu_row_click)
                    .with_children(|row| {
                        row.spawn((
                            Name::new("challenge_row_text"),
                            Text2d::new(def.name.clone()),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(14.0),
                                ..default()
                            },
                            UiTextSize::from(Rh(52.0)),
                            UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                            UiLayout::window()
                                .pos((Rl(8.0), Rl(50.0)))
                                .anchor(Anchor::CENTER_LEFT)
                                .pack(),
                            Pickable::IGNORE,
                        ));
                    });
                }
            });

        // 滚动列表窗口
        panel
            .spawn((
                Name::new("Palette Scroll"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(75.0)))
                    .size((Rl(96.0), Rh(40.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                Pickable::IGNORE,
                TabBlocksRoot,
            ))
            .with_children(|list| {
                for (i, def) in library.defs.iter().enumerate() {
                    let y = i as f32 * ROW_H_PCT + ROW_H_PCT / 2.0;
                    let row_material =
                        materials.add(ColorMaterial::from(Color::srgba(def.color[0], def.color[1], def.color[2], def.color[3])));
                    let swatch_material =
                        materials.add(ColorMaterial::from(Color::srgba(def.color[0], def.color[1], def.color[2], def.color[3])));
                    list.spawn((
                        Name::new(format!("row_{:02}_{}", i, def.id)),
                        UiLayout::window()
                            .pos((Rl(50.0), Rl(y)))
                            .size((Rl(97.0), Rl(ROW_H_PCT * 0.92)))
                            .anchor(Anchor::CENTER)
                            .pack(),
                        UiColor::new(vec![
                            (UiBase::id(), theme.row_base),
                            (UiHover::id(), theme.row_hover),
                            (UiSelected::id(), theme.row_selected),
                        ]),
                        UiHover::new().instant(true),
                        UiSelected(0.0),
                        UiMeshPlane2d,
                        MeshMaterial2d(row_material),
                        PaletteRow(i),
                    ))
                    .observe(hover_set::<Pointer<Over>, true>)
                    .observe(hover_set::<Pointer<Out>, false>)
                    .observe(palette_row_click)
                    .with_children(|row| {
                        // 色块
                        row.spawn((
                            Name::new("swatch"),
                            UiLayout::window()
                                .pos((Rl(8.0), Rl(50.0)))
                                .size((Rl(9.0), Rl(62.0)))
                                .anchor(Anchor::CENTER_LEFT)
                                .pack(),
                            UiMeshPlane2d,
                            MeshMaterial2d(swatch_material),
                            Pickable::IGNORE,
                        ));
                        // 名称
                        row.spawn((
                            Name::new("name"),
                            Text2d::new(def.name.clone()),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            UiTextSize::from(Rh(52.0)),
                            UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                            UiLayout::window()
                                .pos((Rl(19.0), Rl(50.0)))
                                .anchor(Anchor::CENTER_LEFT)
                                .pack(),
                            Pickable::IGNORE,
                        ));
                    });
                }
            });

        // 存档 Tab 内容（默认隐藏；Tab 切换显示）
        panel
            .spawn((
                Name::new("Saves Tab"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(10.5)))
                    .size((Rl(96.0), Rh(84.0)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                Pickable::IGNORE,
                Visibility::Hidden,
                TabSavesRoot,
            ))
            .with_children(|sv| {
                // 行 1：保存 / JSON / 分享（观察者签名各异，逐个生成）
                let save_mat = materials.add(ColorMaterial::from(theme.row_base));
                sv.spawn((
                    Name::new("save_btn"),
                    UiLayout::window()
                        .pos((Rl(16.5), Rh(10.0)))
                        .size((Rl(30.0), Rh(8.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(save_mat),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(save_button_click)
                .with_children(|b| {
                    b.spawn((
                        Name::new("btn_text"),
                        Text2d::new("保存"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(48.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window().full().pack(),
                        Pickable::IGNORE,
                    ));
                });
                let json_mat = materials.add(ColorMaterial::from(theme.row_base));
                sv.spawn((
                    Name::new("json_btn"),
                    UiLayout::window()
                        .pos((Rl(50.0), Rh(10.0)))
                        .size((Rl(30.0), Rh(8.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(json_mat),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(json_button_click)
                .with_children(|b| {
                    b.spawn((
                        Name::new("btn_text"),
                        Text2d::new("JSON"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(48.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window().full().pack(),
                        Pickable::IGNORE,
                    ));
                });
                let share_mat = materials.add(ColorMaterial::from(theme.row_base));
                sv.spawn((
                    Name::new("share_btn"),
                    UiLayout::window()
                        .pos((Rl(83.5), Rh(10.0)))
                        .size((Rl(30.0), Rh(8.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(share_mat),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(share_button_click)
                .with_children(|b| {
                    b.spawn((
                        Name::new("btn_text"),
                        Text2d::new("分享"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(48.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window().full().pack(),
                        Pickable::IGNORE,
                    ));
                });
                // 行 2：加载槽位 / 导入最新
                let load_mat = materials.add(ColorMaterial::from(theme.row_base));
                sv.spawn((
                    Name::new("load_slot_btn"),
                    UiLayout::window()
                        .pos((Rl(25.0), Rh(24.0)))
                        .size((Rl(46.0), Rh(8.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(load_mat),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(load_slot_button_click)
                .with_children(|b| {
                    b.spawn((
                        Name::new("btn_text"),
                        Text2d::new("加载槽位"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(48.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window().full().pack(),
                        Pickable::IGNORE,
                    ));
                });
                let il_mat = materials.add(ColorMaterial::from(theme.row_base));
                sv.spawn((
                    Name::new("import_latest_btn"),
                    UiLayout::window()
                        .pos((Rl(75.0), Rh(24.0)))
                        .size((Rl(46.0), Rh(8.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(il_mat),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(import_latest_button_click)
                .with_children(|b| {
                    b.spawn((
                        Name::new("btn_text"),
                        Text2d::new("导入最新"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(48.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window().full().pack(),
                        Pickable::IGNORE,
                    ));
                });
                // 文件列表标签 + 多行列表
                sv.spawn((
                    Name::new("Save List Label"),
                    Text2d::new("存档文件"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(17.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(2.8)),
                    UiColor::new(vec![(UiBase::id(), theme.accent)]),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(36.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                ));
                sv.spawn((
                    Name::new("Save List Text"),
                    Text2d::new("（暂无存档）"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(2.6)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(41.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                    SaveListText,
                ));
                // 路径导入行：输入框 + 导入按钮
                let box_mat = materials.add(ColorMaterial::from(theme.row_base));
                sv.spawn((
                    Name::new("Path Input Box"),
                    UiLayout::window()
                        .pos((Rl(30.0), Rh(88.0)))
                        .size((Rl(58.0), Rh(7.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiSelected::id(), theme.row_selected),
                    ]),
                    UiSelected(0.0),
                    UiMeshPlane2d,
                    MeshMaterial2d(box_mat),
                    Pickable::default(),
                    PathInputBox,
                ))
                .observe(path_box_click)
                .with_children(|b| {
                    b.spawn((
                        Name::new("Path Input Text"),
                        Text2d::new("输入路径…"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(46.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window()
                            .pos((Rl(6.0), Rl(50.0)))
                            .anchor(Anchor::CENTER_LEFT)
                            .pack(),
                        Pickable::IGNORE,
                        PathInputText,
                    ));
                });
                let imp_mat = materials.add(ColorMaterial::from(theme.row_base));
                sv.spawn((
                    Name::new("Path Import Button"),
                    UiLayout::window()
                        .pos((Rl(72.0), Rh(88.0)))
                        .size((Rl(30.0), Rh(7.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    UiColor::new(vec![
                        (UiBase::id(), theme.row_base),
                        (UiHover::id(), theme.row_hover),
                    ]),
                    UiHover::new().instant(true),
                    UiMeshPlane2d,
                    MeshMaterial2d(imp_mat),
                    Pickable::default(),
                ))
                .observe(hover_set::<Pointer<Over>, true>)
                .observe(hover_set::<Pointer<Out>, false>)
                .observe(path_import_click)
                .with_children(|b| {
                    b.spawn((
                        Name::new("path_import_text"),
                        Text2d::new("导入"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(48.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window().full().pack(),
                        Pickable::IGNORE,
                    ));
                });
            });

        // 图鉴 Tab 内容（默认隐藏）
        panel
            .spawn((
                Name::new("Codex Tab"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(10.5)))
                    .size((Rl(96.0), Rh(84.0)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                Pickable::IGNORE,
                Visibility::Hidden,
                TabCodexRoot,
            ))
            .with_children(|cd| {
                // 标题
                cd.spawn((
                    Name::new("Codex Title"),
                    Text2d::new("积木图鉴"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(3.0)),
                    UiColor::new(vec![(UiBase::id(), theme.accent)]),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(2.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                ));
                // 详情区（名称 + 文化描述）
                let detail_mat = materials.add(ColorMaterial::from(theme.row_base));
                cd.spawn((
                    Name::new("Codex Detail"),
                    UiLayout::window()
                        .pos((Rl(50.0), Rh(13.0)))
                        .size((Rl(94.0), Rh(26.0)))
                        .anchor(Anchor::TOP_CENTER)
                        .pack(),
                    UiMeshPlane2d,
                    MeshMaterial2d(detail_mat),
                    Pickable::IGNORE,
                ))
                .with_children(|d| {
                    d.spawn((
                        Name::new("Codex Detail Text"),
                        Text2d::new("点击条目查看文化描述"),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        UiTextSize::from(Rh(3.4)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        UiLayout::window()
                            .pos((Rl(4.0), Rh(4.0)))
                            .anchor(Anchor::TOP_LEFT)
                            .pack(),
                        Pickable::IGNORE,
                        CodexDetailText,
                    ));
                });
                // 条目列表窗口
                cd.spawn((
                    Name::new("Codex List"),
                    UiLayout::window()
                        .pos((Rl(50.0), Rh(62.0)))
                        .size((Rl(96.0), Rh(68.0)))
                        .anchor(Anchor::CENTER)
                        .pack(),
                    Pickable::IGNORE,
                ))
                .with_children(|list| {
                    for (i, def) in library.defs.iter().enumerate() {
                        let y = i as f32 * ROW_H_PCT + ROW_H_PCT / 2.0;
                        let row_mat = materials.add(ColorMaterial::from(theme.row_base));
                        let unlocked = collection.codex.contains(&def.id);
                        let label = if unlocked {
                            format!("{}  ✓", def.name)
                        } else {
                            format!("{}  🔒", def.name)
                        };
                        list.spawn((
                            Name::new(format!("codex_row_{:02}_{}", i, def.id)),
                            UiLayout::window()
                                .pos((Rl(50.0), Rl(y)))
                                .size((Rl(97.0), Rl(ROW_H_PCT * 0.92)))
                                .anchor(Anchor::CENTER)
                                .pack(),
                            UiColor::new(vec![
                                (
                                    UiBase::id(),
                                    if unlocked { theme.text_main } else { theme.text_dim },
                                ),
                                (UiHover::id(), theme.text_main),
                            ]),
                            UiHover::new().instant(true),
                            UiMeshPlane2d,
                            MeshMaterial2d(row_mat),
                            Text2d::new(label),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(18.0),
                                ..default()
                            },
                            UiTextSize::from(Rh(52.0)),
                            CodexRow(i),
                        ))
                        .observe(hover_set::<Pointer<Over>, true>)
                        .observe(hover_set::<Pointer<Out>, false>)
                        .observe(codex_row_click);
                    }
                });
            });

        // 成就 Tab 内容（默认隐藏）
        panel
            .spawn((
                Name::new("Achievements Tab"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(10.5)))
                    .size((Rl(96.0), Rh(84.0)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                Pickable::IGNORE,
                Visibility::Hidden,
                TabAchievementsRoot,
            ))
            .with_children(|ac| {
                ac.spawn((
                    Name::new("Achievements Title"),
                    Text2d::new("成就"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(3.0)),
                    UiColor::new(vec![(UiBase::id(), theme.accent)]),
                    UiLayout::window()
                        .pos((Rl(4.0), Rh(2.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                ));
                for (i, (id, name, desc)) in achievement_defs().iter().enumerate() {
                    let unlocked = collection.achievements.contains(*id);
                    let card_mat = materials.add(ColorMaterial::from(theme.row_base));
                    let y = 10.0 + i as f32 * 17.0;
                    ac.spawn((
                        Name::new(format!("ach_{id}")),
                        UiLayout::window()
                            .pos((Rl(50.0), Rh(y)))
                            .size((Rl(94.0), Rh(14.5)))
                            .anchor(Anchor::TOP_CENTER)
                            .pack(),
                        UiMeshPlane2d,
                        MeshMaterial2d(card_mat),
                        Pickable::IGNORE,
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Name::new("ach_text"),
                            Text2d::new(format!(
                                "{}  {}\n{}",
                                name,
                                if unlocked { "✓" } else { "🔒" },
                                desc
                            )),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            UiTextSize::from(Rh(3.0)),
                            UiColor::new(vec![(
                                UiBase::id(),
                                if unlocked { theme.accent } else { theme.text_main },
                            )]),
                            UiLayout::window()
                                .pos((Rl(4.0), Rh(2.0)))
                                .anchor(Anchor::TOP_LEFT)
                                .pack(),
                            Pickable::IGNORE,
                            AchievementNameText(i),
                        ));
                    });
                }
            });
    });
}

/// 点击行 → 选中积木（B1 交互；与键盘 1-9/Q/E 共用 BlockLibrary.current）
fn palette_row_click(
    trigger: On<Pointer<Click>>,
    mut library: ResMut<BlockLibrary>,
    rows: Query<&PaletteRow>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let Ok(row) = rows.get(trigger.event_target()) else {
        return;
    };
    library.current = row.0;
}

/// 选中高亮同步：BlockLibrary.current 变化 → 更新各行的 UiSelected
fn palette_sync_selection(library: Res<BlockLibrary>, mut rows: Query<(&PaletteRow, &mut UiSelected)>) {
    if !library.is_changed() {
        return;
    }
    for (row, mut sel) in &mut rows {
        let target = (row.0 == library.current) as u8 as f32;
        if (sel.0 - target).abs() > f32::EPSILON {
            sel.0 = target;
        }
    }
}

/// 滚轮滚动（B1）：仅指针悬停于某行时生效；更新行位置与可见性。
/// 首次运行（offset 初始 0）也会应用一次——隐藏窗口外的行，避免越界绘制
/// （lunex 不裁剪子节点，越界行必须显式 Hidden）。
#[allow(clippy::type_complexity)]
fn palette_scroll_system(
    mut scroll: ResMut<PaletteScroll>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    hover_map: Res<bevy::picking::hover::HoverMap>,
    library: Res<BlockLibrary>,
    mut rows: Query<(Entity, &PaletteRow, &mut UiLayout, &mut Visibility)>,
    mut last_applied: Local<Option<f32>>,
) {
    let delta = mouse_scroll.delta.y;
    if delta != 0.0 {
        let over_palette = hover_map
            .get(&PointerId::Mouse)
            .is_some_and(|hits| hits.keys().any(|e| rows.iter().any(|(ent, ..)| ent == *e)));
        if over_palette {
            let max = (library.defs.len() as f32 - PALETTE_VISIBLE as f32).max(0.0);
            // 滚轮向上（delta.y > 0）→ 列表上移（offset 减小）；方向与平台一致
            scroll.0 = (scroll.0 - delta * 0.3).clamp(0.0, max);
        }
    }
    if *last_applied == Some(scroll.0) {
        return;
    }
    *last_applied = Some(scroll.0);
    let offset = scroll.0;
    for (_, row, mut layout, mut vis) in &mut rows {
        let y = (row.0 as f32 - offset) * ROW_H_PCT + ROW_H_PCT / 2.0;
        if let Some(bevy_lunex::UiLayoutType::Window(w)) =
            layout.layouts.get_mut(&UiBase::id())
        {
            w.pos = (Rl(50.0), Rl(y)).into();
        }
        *vis = if y < 0.0 || y > 100.0 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

// ======================================================================
// B2：蓝图模式区（完成度进度条 + 主题下拉）
// ======================================================================

/// 进度条填充（宽度 = 完成度 %）
#[derive(Component)]
pub struct ProgressFill;

/// 「42%」完成度文本
#[derive(Component)]
pub struct ProgressText;

/// 主题按钮文本（当前主题名）
#[derive(Component)]
pub struct ThemeButtonText;

/// 主题下拉行（存主题索引）
#[derive(Component)]
pub struct ThemeMenuRow(pub usize);

/// 主题下拉开合状态
#[derive(Resource, Default)]
pub struct ThemeMenuOpen(pub bool);

/// 完成度变化 → 进度条填充宽度 + 文本
fn b2_progress_sync(
    blueprint: Res<Blueprint>,
    mut fills: Query<&mut UiLayout, With<ProgressFill>>,
    mut texts: Query<&mut Text2d, With<ProgressText>>,
) {
    if !blueprint.is_changed() {
        return;
    }
    let pct = (blueprint.completion.clamp(0.0, 1.0) * 100.0).round();
    for mut fill in &mut fills {
        if let Some(bevy_lunex::UiLayoutType::Window(w)) =
            fill.layouts.get_mut(&UiBase::id())
        {
            w.size = (Rl(pct), Rl(82.0)).into();
        }
    }
    for mut t in &mut texts {
        t.0 = format!("{pct:.0}%");
    }
}

/// 主题按钮点击 → 开合下拉
fn theme_button_click(
    trigger: On<Pointer<Click>>,
    mut menu: ResMut<ThemeMenuOpen>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    menu.0 = !menu.0;
}

/// 下拉开合状态 → 行可见性
fn b2_theme_menu_sync(
    menu: Res<ThemeMenuOpen>,
    mut rows: Query<&mut Visibility, With<ThemeMenuRow>>,
) {
    if !menu.is_changed() {
        return;
    }
    for mut v in &mut rows {
        *v = if menu.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// 下拉行点击 → 切换主题（与 egui 面板/教程共用 select_blueprint 流程）
fn theme_menu_row_click(
    trigger: On<Pointer<Click>>,
    rows: Query<&ThemeMenuRow>,
    mut blueprint: ResMut<Blueprint>,
    mut library: ResMut<BlueprintLibrary>,
    mut menu: ResMut<ThemeMenuOpen>,
    mut commands: Commands,
    ghosts: Query<Entity, With<BlueprintGhost>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let Ok(row) = rows.get(trigger.event_target()) else {
        return;
    };
    select_blueprint(&mut blueprint, &mut library, row.0);
    // 切换后销毁旧幽灵蓝图，由对账系统按新蓝图重建（与 block_panel 一致）
    for e in &ghosts {
        commands.entity(e).despawn();
    }
    menu.0 = false;
    info!("🏯 主题切换：{}", library.current_def().name);
}

/// 主题变化（下拉/其他入口）→ 按钮文本
fn b2_theme_name_sync(
    library: Res<BlueprintLibrary>,
    mut texts: Query<&mut Text2d, With<ThemeButtonText>>,
) {
    if !library.is_changed() {
        return;
    }
    for mut t in &mut texts {
        t.0 = library.current_def().name.clone();
    }
}

// ======================================================================
// B6：成就 Tab（5 项状态）
// ======================================================================

/// 成就内容根标记
#[derive(Component)]
pub struct TabAchievementsRoot;

/// 成就卡片文本（存成就索引 0..5）
#[derive(Component)]
pub struct AchievementNameText(pub usize);

/// 成就状态同步：Collection 变化 → 成就卡片高亮 + 文本
fn b6_achievement_sync(
    collection: Res<Collection>,
    theme: Res<LunexTheme>,
    mut names: Query<(&AchievementNameText, &mut Text2d, &mut UiColor)>,
) {
    if !collection.is_changed() {
        return;
    }
    for (row, mut t, mut color) in &mut names {
        let (id, name, desc) = achievement_defs()[row.0];
        let unlocked = collection.achievements.contains(id);
        t.0 = format!(
            "{}  {}\n{}",
            name,
            if unlocked { "✓" } else { "🔒" },
            desc
        );
        *color = UiColor::new(vec![(
            UiBase::id(),
            if unlocked { theme.accent } else { theme.text_main },
        )]);
    }
}

// ======================================================================
// B7：知识卡片（PRD §3.3 智能提示；toast 展示，蓝图模式停顿触发）
// ======================================================================

/// 知识卡片容器
#[derive(Component)]
pub struct KnowledgeCardRoot;

/// 卡片标题文本
#[derive(Component)]
pub struct KnowledgeCardName;

/// 卡片描述文本
#[derive(Component)]
pub struct KnowledgeCardDesc;

/// 知识卡片同步：KnowledgeHints.card 变化 → 显隐 + 文本
fn b7_knowledge_sync(
    hints: Res<KnowledgeHints>,
    mut cards: Query<&mut Visibility, With<KnowledgeCardRoot>>,
    mut texts: ParamSet<(
        Query<&mut Text2d, With<KnowledgeCardName>>,
        Query<&mut Text2d, With<KnowledgeCardDesc>>,
    )>,
) {
    if !hints.is_changed() {
        return;
    }
    for mut v in &mut cards {
        *v = if hints.card.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Some(card) = &hints.card {
        for mut t in texts.p0() {
            t.0 = format!("📖 {}", card.name);
        }
        for mut t in texts.p1() {
            t.0 = card.desc.clone();
        }
    }
}

// ======================================================================
// B8：蓝图透明度滑杆（幽灵蓝图 alpha 0.1..0.8）
// ======================================================================

/// 滑杆轨道
#[derive(Component)]
pub struct OpacitySlider;

/// 滑杆填充
#[derive(Component)]
pub struct OpacityFill;

/// 滑杆滑块
#[derive(Component)]
pub struct OpacityKnob;

/// 滑杆拖拽状态（按下 → Move 更新 → 释放清除）
#[derive(Resource, Default)]
pub struct OpacityDragging(pub bool);

/// 依命中世界 x 计算 alpha（0.1..0.8）
fn opacity_from_hit(
    hit_x: f32,
    tracks: &Query<(&GlobalTransform, &Dimension), With<OpacitySlider>>,
    target: Entity,
    alpha: &mut BlueprintAlpha,
) {
    let Ok((tf, dim)) = tracks.get(target) else {
        return;
    };
    let min = tf.translation().x - dim.x / 2.0;
    let frac = ((hit_x - min) / dim.x).clamp(0.0, 1.0);
    alpha.value = 0.1 + frac * 0.7;
}

/// 按下：定位 + 进入拖拽
fn opacity_press(
    trigger: On<Pointer<Press>>,
    mut alpha: ResMut<BlueprintAlpha>,
    mut dragging: ResMut<OpacityDragging>,
    tracks: Query<(&GlobalTransform, &Dimension), With<OpacitySlider>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    opacity_from_hit(
        trigger.event().hit.position.unwrap_or(Vec3::ZERO).x,
        &tracks,
        trigger.event_target(),
        &mut alpha,
    );
    dragging.0 = true;
}

/// 拖拽中移动：持续更新
fn opacity_move(
    trigger: On<Pointer<Move>>,
    mut alpha: ResMut<BlueprintAlpha>,
    dragging: Res<OpacityDragging>,
    tracks: Query<(&GlobalTransform, &Dimension), With<OpacitySlider>>,
) {
    if !dragging.0 {
        return;
    }
    opacity_from_hit(
        trigger.event().hit.position.unwrap_or(Vec3::ZERO).x,
        &tracks,
        trigger.event_target(),
        &mut alpha,
    );
}

/// 释放：退出拖拽
fn opacity_release(trigger: On<Pointer<Release>>, mut dragging: ResMut<OpacityDragging>) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    dragging.0 = false;
}

/// alpha 变化 → 填充宽度 + 滑块位置
fn b8_opacity_sync(
    alpha: Res<BlueprintAlpha>,
    mut nodes: Query<(&mut UiLayout, Option<&OpacityFill>, Option<&OpacityKnob>)>,
) {
    if !alpha.is_changed() {
        return;
    }
    let frac = ((alpha.value - 0.1) / 0.7).clamp(0.0, 1.0);
    for (mut layout, fill, knob) in &mut nodes {
        let Some(bevy_lunex::UiLayoutType::Window(w)) = layout.layouts.get_mut(&UiBase::id())
        else {
            continue;
        };
        if fill.is_some() {
            w.size = (Rl(frac * 100.0), Rl(80.0)).into();
        }
        if knob.is_some() {
            w.pos = (Rl(frac * 100.0), Rl(50.0)).into();
        }
    }
}

/// alpha 变化 → 幽灵蓝图材质透明度（与 egui 面板同一效果）
fn b8_opacity_apply(
    alpha: Res<BlueprintAlpha>,
    render: Res<BlockRenderAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !alpha.is_changed() {
        return;
    }
    for handle in render.blueprint_materials.values() {
        if let Some(mut mat) = materials.get_mut(handle) {
            mat.base_color.set_alpha(alpha.value);
        }
    }
}

// ======================================================================
// B5：图鉴 Tab（35 条目解锁状态 + 文化描述）
// ======================================================================

/// 图鉴内容根标记
#[derive(Component)]
pub struct TabCodexRoot;

/// 图鉴行（存积木索引）
#[derive(Component)]
pub struct CodexRow(pub usize);

/// 图鉴滚动偏移（行单位）
#[derive(Resource, Default)]
pub struct CodexScroll(pub f32);

/// 图鉴选中条目
#[derive(Resource, Default)]
pub struct CodexSelected(pub Option<usize>);

/// 图鉴详情文本（名称 + 文化描述）
#[derive(Component)]
pub struct CodexDetailText;

/// 图鉴行点击 → 选中
fn codex_row_click(
    trigger: On<Pointer<Click>>,
    rows: Query<&CodexRow>,
    mut selected: ResMut<CodexSelected>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let Ok(row) = rows.get(trigger.event_target()) else {
        return;
    };
    selected.0 = Some(row.0);
}

/// 图鉴详情同步：选中变化 → 详情文本
fn b5_codex_detail_sync(
    selected: Res<CodexSelected>,
    collection: Res<Collection>,
    library: Res<BlockLibrary>,
    mut texts: Query<&mut Text2d, With<CodexDetailText>>,
) {
    if !selected.is_changed() {
        return;
    }
    let txt = match selected.0 {
        Some(i) if i < library.defs.len() => {
            let def = &library.defs[i];
            let unlocked = collection.codex.contains(&def.id);
            format!(
                "{}  {}\n\n{}",
                def.name,
                if unlocked { "✓ 已解锁" } else { "🔒 未解锁" },
                def.description
            )
        }
        _ => "点击条目查看文化描述".to_string(),
    };
    for mut t in &mut texts {
        t.0 = txt.clone();
    }
}

/// 图鉴列表滚动（同积木面板模式：悬停于行上滚轮生效；窗口外行隐藏）
#[allow(clippy::type_complexity)]
fn codex_scroll_system(
    mut scroll: ResMut<CodexScroll>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    hover_map: Res<bevy::picking::hover::HoverMap>,
    library: Res<BlockLibrary>,
    mut rows: Query<(Entity, &CodexRow, &mut UiLayout, &mut Visibility)>,
    mut last_applied: Local<Option<f32>>,
) {
    let delta = mouse_scroll.delta.y;
    if delta != 0.0 {
        let over = hover_map
            .get(&PointerId::Mouse)
            .is_some_and(|hits| hits.keys().any(|e| rows.iter().any(|(ent, ..)| ent == *e)));
        if over {
            let max = (library.defs.len() as f32 - PALETTE_VISIBLE as f32).max(0.0);
            scroll.0 = (scroll.0 - delta * 0.3).clamp(0.0, max);
        }
    }
    if *last_applied == Some(scroll.0) {
        return;
    }
    *last_applied = Some(scroll.0);
    let offset = scroll.0;
    for (_, row, mut layout, mut vis) in &mut rows {
        let y = (row.0 as f32 - offset) * ROW_H_PCT + ROW_H_PCT / 2.0;
        if let Some(bevy_lunex::UiLayoutType::Window(w)) =
            layout.layouts.get_mut(&UiBase::id())
        {
            w.pos = (Rl(50.0), Rl(y)).into();
        }
        *vis = if y < 0.0 || y > 100.0 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// 图鉴行解锁状态同步：Collection 变化 → 行名/状态文本与颜色
fn b5_codex_status_sync(
    collection: Res<Collection>,
    library: Res<BlockLibrary>,
    theme: Res<LunexTheme>,
    mut rows: Query<(&CodexRow, &mut Text2d, &mut UiColor)>,
) {
    if !collection.is_changed() {
        return;
    }
    for (row, mut t, mut color) in &mut rows {
        if row.0 >= library.defs.len() {
            continue;
        }
        let def = &library.defs[row.0];
        let unlocked = collection.codex.contains(&def.id);
        t.0 = if unlocked {
            format!("{}  ✓", def.name)
        } else {
            format!("{}  🔒", def.name)
        };
        *color = UiColor::new(vec![(
            UiBase::id(),
            if unlocked {
                theme.text_main
            } else {
                theme.text_dim
            },
        )]);
    }
}

// ======================================================================
// A4/B4：面板 Tab 架构（积木 / 图鉴 / 成就 / 存档）+ 存档 Tab
// ======================================================================

/// 面板 Tab
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LunexTabId {
    Blocks,
    Codex,
    Achievements,
    Saves,
}

/// 当前面板 Tab
#[derive(Resource)]
pub struct LunexTab(pub LunexTabId);
impl Default for LunexTab {
    fn default() -> Self {
        Self(LunexTabId::Blocks)
    }
}

/// Tab 按钮（存目标 Tab）
#[derive(Component)]
pub struct TabButton(pub LunexTabId);

/// 积木 Tab 内容根标记（标题/B2/B3/列表 4 个实体共用）
#[derive(Component)]
pub struct TabBlocksRoot;

/// 存档 Tab 内容根标记
#[derive(Component)]
pub struct TabSavesRoot;

/// Tab 点击 → 切换
fn tab_button_click(
    trigger: On<Pointer<Click>>,
    mut tab: ResMut<LunexTab>,
    buttons: Query<&TabButton>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let Ok(btn) = buttons.get(trigger.event_target()) else {
        return;
    };
    tab.0 = btn.0;
}

/// Tab 切换 → 各内容根可见性 + Tab 按钮高亮（UiSelected）
fn lunex_tab_sync(
    tab: Res<LunexTab>,
    mut contents: Query<
        (
            &mut Visibility,
            Option<&TabBlocksRoot>,
            Option<&TabSavesRoot>,
            Option<&TabCodexRoot>,
            Option<&TabAchievementsRoot>,
        ),
        Or<(
            With<TabBlocksRoot>,
            With<TabSavesRoot>,
            With<TabCodexRoot>,
            With<TabAchievementsRoot>,
        )>,
    >,
    mut buttons: Query<(&TabButton, &mut UiSelected)>,
) {
    if !tab.is_changed() {
        return;
    }
    for (mut v, blocks, saves, codex, achievements) in &mut contents {
        let visible = (blocks.is_some() && tab.0 == LunexTabId::Blocks)
            || (saves.is_some() && tab.0 == LunexTabId::Saves)
            || (codex.is_some() && tab.0 == LunexTabId::Codex)
            || (achievements.is_some() && tab.0 == LunexTabId::Achievements);
        *v = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (btn, mut sel) in &mut buttons {
        let target = (btn.0 == tab.0) as u8 as f32;
        if (sel.0 - target).abs() > f32::EPSILON {
            sel.0 = target;
        }
    }
}

// ---- 存档 Tab ----

/// 存档文件列表文本（多行）
#[derive(Component)]
pub struct SaveListText;

/// 路径输入框（可点击聚焦）
#[derive(Component)]
pub struct PathInputBox;

/// 路径输入框文本
#[derive(Component)]
pub struct PathInputText;

/// 自定义路径导入输入状态
#[derive(Resource, Default)]
pub struct PathInput {
    pub value: String,
    pub focused: bool,
}

/// 存档列表：每 1 秒刷新（仅存档 Tab 激活时）
fn save_list_system(
    time: Res<Time>,
    tab: Res<LunexTab>,
    mut last: Local<f32>,
    mut texts: Query<&mut Text2d, With<SaveListText>>,
) {
    if tab.0 != LunexTabId::Saves {
        return;
    }
    if time.elapsed_secs() - *last < 1.0 {
        return;
    }
    *last = time.elapsed_secs();
    let entries = save_file_list(&saves_dir());
    let txt = if entries.is_empty() {
        "（暂无存档）".to_string()
    } else {
        entries
            .iter()
            .map(|(n, b, t)| format!("{n} · {b} 块 · {t}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    for mut t in &mut texts {
        t.0 = txt.clone();
    }
}

/// 路径输入：聚焦时接收键盘（字符/退格/Esc/Enter）
fn path_input_system(
    mut input: ResMut<PathInput>,
    mut keys: MessageReader<KeyboardInput>,
    mut texts: Query<&mut Text2d, With<PathInputText>>,
    mut boxes: Query<&mut UiSelected, With<PathInputBox>>,
) {
    if input.focused {
        for msg in keys.read() {
            if msg.state != ButtonState::Pressed {
                continue;
            }
            match &msg.logical_key {
                Key::Character(c) => input.value.push_str(c),
                Key::Backspace => {
                    input.value.pop();
                }
                Key::Escape => input.focused = false,
                Key::Enter => input.focused = false,
                _ => {}
            }
        }
    }
    for mut t in &mut texts {
        t.0 = if input.value.is_empty() {
            "输入路径…".to_string()
        } else {
            input.value.clone()
        };
    }
    for mut s in &mut boxes {
        s.0 = if input.focused { 1.0 } else { 0.0 };
    }
}

/// 路径输入框点击 → 聚焦
fn path_box_click(trigger: On<Pointer<Click>>, mut input: ResMut<PathInput>) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    input.focused = true;
}

/// 导入自定义路径存档
fn path_import_click(
    trigger: On<Pointer<Click>>,
    input: Res<PathInput>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed: Query<Entity, With<PlacedBlock>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let path_str = input.value.trim();
    if path_str.is_empty() {
        info!("💾 请先输入存档路径");
        return;
    }
    match load_save_from_path(Path::new(path_str)) {
        Ok(save) => {
            let loaded = import_save(
                &mut commands,
                &mut stack,
                &library,
                &render,
                &mut blueprint,
                &mut challenge,
                &placed,
                save,
            );
            info!("📂 已导入 {path_str}（{loaded} 个积木）");
        }
        Err(e) => error!("导入失败: {e}"),
    }
}

/// 保存按钮（F5 同流程）
fn save_button_click(
    trigger: On<Pointer<Click>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<Blueprint>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = save_slot(&stack, &blueprint) {
        error!("保存失败: {e}");
    }
}

/// JSON 导出按钮（F6 同流程）
fn json_button_click(
    trigger: On<Pointer<Click>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<Blueprint>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = export_json(&stack, &blueprint) {
        error!("JSON 导出失败: {e}");
    }
}

/// 分享按钮（F7 同流程）
fn share_button_click(
    trigger: On<Pointer<Click>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<Blueprint>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = share_export(&stack, &blueprint) {
        error!("分享导出失败: {e}");
    }
}

/// 加载槽位按钮（F9 同流程）
fn load_slot_button_click(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed: Query<Entity, With<PlacedBlock>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = load_slot(
        &mut commands,
        &mut stack,
        &library,
        &render,
        &mut blueprint,
        &mut challenge,
        &placed,
    ) {
        info!("💾 {e}");
    }
}

/// 导入最新分享按钮（F8 同流程）
fn import_latest_button_click(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed: Query<Entity, With<PlacedBlock>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = import_latest(
        &mut commands,
        &mut stack,
        &library,
        &render,
        &mut blueprint,
        &mut challenge,
        &placed,
    ) {
        info!("📂 {e}");
    }
}

// ======================================================================
// B3：挑战区（选择下拉 + 状态/倒计时/材料 + 开始/重试）
// ======================================================================

/// 挑战名（下拉按钮文本）
#[derive(Component)]
pub struct ChallengeNameText;

/// 状态行文本（倒计时/材料/结果）
#[derive(Component)]
pub struct ChallengeStatusText;

/// 开始/重试按钮文本
#[derive(Component)]
pub struct ChallengeButtonText;

/// 开始按钮标记（进行中禁用）
#[derive(Component)]
pub struct ChallengeStartButton;

/// 挑战下拉行（存挑战索引）
#[derive(Component)]
pub struct ChallengeMenuRow(pub usize);

/// 挑战下拉开合状态
#[derive(Resource, Default)]
pub struct ChallengeMenuOpen(pub bool);

/// 挑战按钮点击 → 开合下拉
fn challenge_button_click(trigger: On<Pointer<Click>>, mut menu: ResMut<ChallengeMenuOpen>) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    menu.0 = !menu.0;
}

/// 下拉开合 → 行可见性
fn b3_challenge_menu_sync(
    menu: Res<ChallengeMenuOpen>,
    mut rows: Query<&mut Visibility, With<ChallengeMenuRow>>,
) {
    if !menu.is_changed() {
        return;
    }
    for mut v in &mut rows {
        *v = if menu.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// 下拉行点击 → 切换挑战（select_challenge 重置为 Idle）
fn challenge_menu_row_click(
    trigger: On<Pointer<Click>>,
    rows: Query<&ChallengeMenuRow>,
    mut challenge: ResMut<Challenge>,
    mut library: ResMut<ChallengeLibrary>,
    mut menu: ResMut<ChallengeMenuOpen>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let Ok(row) = rows.get(trigger.event_target()) else {
        return;
    };
    select_challenge(&mut challenge, &mut library, row.0);
    menu.0 = false;
    info!("🏆 挑战选择：{}", library.current_def().name);
}

/// 开始/重试按钮：挑战非进行中时启动（清空世界，与 C 键同一流程）
fn challenge_start_click(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    mut blueprint: ResMut<Blueprint>,
    mut blueprint_library: ResMut<BlueprintLibrary>,
    mut challenge: ResMut<Challenge>,
    placed: Query<Entity, With<PlacedBlock>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if challenge.is_active() {
        return;
    }
    start_challenge(
        &mut commands,
        &mut stack,
        &mut blueprint,
        &mut blueprint_library,
        &mut challenge,
        &placed,
    );
    info!("🏆 挑战开始：{}", challenge.def.name);
}

/// 挑战状态/时间/材料变化 → 状态行 + 按钮文本（进行中每帧 tick，文本更新开销可忽略）
fn b3_challenge_sync(
    challenge: Res<Challenge>,
    mut texts: ParamSet<(
        Query<&mut Text2d, With<ChallengeStatusText>>,
        Query<&mut Text2d, With<ChallengeButtonText>>,
    )>,
) {
    if !challenge.is_changed() {
        return;
    }
    let (used, total) = challenge.quota_used_total();
    let (status_line, button_label) = match challenge.state {
        ChallengeState::Active => (
            format!("⏱ {:.0}s  🧱 {used}/{total}", challenge.time_left),
            "进行中".to_string(),
        ),
        ChallengeState::Won => (
            format!("★ {}（再来一局）", "★".repeat(challenge.stars as usize)),
            "再来一局".to_string(),
        ),
        ChallengeState::Failed => ("⏱ 失败（重试）".to_string(), "重试".to_string()),
        ChallengeState::Idle => ("未开始".to_string(), "开始".to_string()),
    };
    for mut t in texts.p0() {
        t.0 = status_line.clone();
    }
    for mut t in texts.p1() {
        t.0 = button_label.clone();
    }
}

/// 挑战库变化（下拉/其他入口）→ 名称文本 + 收起菜单
fn b3_challenge_name_sync(
    library: Res<ChallengeLibrary>,
    mut names: Query<&mut Text2d, With<ChallengeNameText>>,
    mut menu: ResMut<ChallengeMenuOpen>,
) {
    if !library.is_changed() {
        return;
    }
    menu.0 = false;
    for mut t in &mut names {
        t.0 = library.current_def().name.clone();
    }
}

/// 冒烟诊断：打印 lunex 布局、2D 文本排布、3D 文本网格的实际状态。
fn ui_probe_diagnostic(
    time: Res<Time>,
    mut done: Local<bool>,
    roots: Query<&Dimension, (With<UiLayoutRoot>, With<UiFetchFromCamera<0>>)>,
    banners: Query<
        (Entity, Option<&Mesh2d>, Option<&MeshMaterial2d<ColorMaterial>>),
        With<UiMeshPlane2d>,
    >,
    texts: Query<(&Text2d, &TextLayoutInfo), With<UiTextSize>>,
    probes: Query<
        (
            Entity,
            &Transform,
            Option<&Mesh3d>,
            Option<&bevy_rich_text3d::Text3dDimensionOut>,
        ),
        With<Text3dProbe>,
    >,
    palette: Query<(&PaletteRow, &UiSelected, &Visibility)>,
    theme_texts: Query<&Text2d, With<ThemeButtonText>>,
    progress_texts: Query<&Text2d, With<ProgressText>>,
    menu_rows: Query<(&ThemeMenuRow, &Visibility)>,
    challenge_status: Query<&Text2d, With<ChallengeStatusText>>,
    challenge_buttons: Query<&Text2d, With<ChallengeButtonText>>,
    challenge_names: Query<&Text2d, With<ChallengeNameText>>,
    renderer: Option<Res<bevy_rich_text3d::TextRenderer>>,
) {
    if std::env::var("PHOENIX_UI_PROBE").is_err() || *done || time.elapsed_secs() < 4.0 {
        return;
    }
    *done = true;
    info!(
        "🧪 rich_text3d TextRenderer resource exists: {}",
        renderer.is_some()
    );
    for d in &roots {
        info!("🧪 lunex root dimension = {d:?}");
    }
    for (e, mesh, mat) in &banners {
        info!(
            "🧪 lunex plane {e:?}: mesh2d={} material2d={}",
            mesh.is_some(),
            mat.is_some()
        );
    }
    for (t, layout) in &texts {
        info!("🧪 lunex text \"{}\" layout={:?}", t.0, layout.size);
    }
    for (e, tf, mesh, dim) in &probes {
        info!(
            "🧪 text3d probe {e:?} at {:?}: mesh3d={} dim={:?}",
            tf.translation,
            mesh.is_some(),
            dim.map(|d| d.dimension)
        );
    }
    if !palette.is_empty() {
        let mut rows: Vec<_> = palette.iter().map(|(r, s, v)| (r.0, s.0, *v)).collect();
        rows.sort_by_key(|(i, ..)| *i);
        let visible = rows.iter().filter(|(_, _, v)| *v != Visibility::Hidden).count();
        info!(
            "🧪 palette rows: {} total / {} visible; selected = {:?}",
            rows.len(),
            visible,
            rows.iter().find(|(_, s, _)| *s > 0.5).map(|(i, _, _)| *i)
        );
    }
    for t in &theme_texts {
        info!("🧪 theme button: \"{}\"", t.0);
    }
    for t in &progress_texts {
        info!("🧪 progress text: \"{}\"", t.0);
    }
    let open = menu_rows.iter().filter(|(_, v)| *v != Visibility::Hidden).count();
    info!("🧪 theme menu rows visible: {open}");
    for t in &challenge_names {
        info!("🧪 challenge name: \"{}\"", t.0);
    }
    for t in &challenge_status {
        info!("🧪 challenge status: \"{}\"", t.0);
    }
    for t in &challenge_buttons {
        info!("🧪 challenge button: \"{}\"", t.0);
    }
}

fn ui_probe_tabs(
    time: Res<Time>,
    mut done: Local<bool>,
    tabs: Query<(&TabButton, &UiSelected)>,
    saves_vis: Query<&Visibility, With<TabSavesRoot>>,
    codex_rows: Query<(&CodexRow, &Visibility)>,
    codex_detail: Query<&Text2d, With<CodexDetailText>>,
    codex_vis: Query<&Visibility, With<TabCodexRoot>>,
    ach_vis: Query<&Visibility, With<TabAchievementsRoot>>,
    ach_texts: Query<(&AchievementNameText, &Text2d)>,
    kcard_vis: Query<&Visibility, With<KnowledgeCardRoot>>,
    kcard_names: Query<&Text2d, With<KnowledgeCardName>>,
    op_fills: Query<&UiLayout, With<OpacityFill>>,
    plaques: Query<(Entity, Option<&Mesh3d>), With<PlaqueText>>,
) {
    if std::env::var("PHOENIX_UI_PROBE").is_err() || *done || time.elapsed_secs() < 4.0 {
        return;
    }
    *done = true;
    let mut tabs: Vec<_> = tabs.iter().map(|(b, s)| (format!("{:?}", b.0), s.0)).collect();
    tabs.sort_by(|a, b| a.0.cmp(&b.0));
    info!("🧪 tabs: {:?}", tabs);
    for v in &saves_vis {
        info!("🧪 saves tab visible: {}", *v != Visibility::Hidden);
    }
    if !codex_rows.is_empty() {
        let vis = codex_rows
            .iter()
            .filter(|(_, v)| *v != Visibility::Hidden)
            .count();
        info!("🧪 codex rows: {} total / {vis} visible", codex_rows.iter().count());
        let first: Vec<_> = codex_rows.iter().take(3).map(|(r, v)| (r.0, *v)).collect();
        info!("🧪 codex first rows: {:?}", first);
    }
    for t in &codex_detail {
        info!("🧪 codex detail: \"{}\"", t.0);
    }
    for v in &codex_vis {
        info!("🧪 codex tab visible: {}", *v != Visibility::Hidden);
    }
    for v in &ach_vis {
        info!("🧪 achievements tab visible: {}", *v != Visibility::Hidden);
    }
    let mut ach: Vec<_> = ach_texts
        .iter()
        .map(|(r, t)| (r.0, t.0.clone()))
        .collect();
    ach.sort_by_key(|(i, _)| *i);
    for (i, t) in ach.iter().take(2) {
        info!("🧪 achievement[{i}]: \"{}\"", t.split('\n').next().unwrap_or(""));
    }
    for v in &kcard_vis {
        info!("🧪 knowledge card visible: {}", *v != Visibility::Hidden);
    }
    for t in &kcard_names {
        if !t.0.is_empty() {
            info!("🧪 knowledge card: \"{}\"", t.0);
        }
    }
    for f in &op_fills {
        let w = f
            .layouts
            .get(&UiBase::id())
            .and_then(|l| match l {
                bevy_lunex::UiLayoutType::Window(w) => Some(w.size),
                _ => None,
            });
        info!("🧪 opacity fill size: {:?}", w);
    }
    for (e, m) in &plaques {
        info!("🧪 plaque text3d {e:?}: mesh={}", m.is_some());
    }
}


/// 2D UI 相机：叠加在 3D 主相机之上（order 更高）、透明清屏，
/// 作为 lunex 布局的尺寸来源（`UiSourceCamera::<0>`）。
///
/// 说明：bevy_ui 的 HUD 提示（hud.rs）会改由这台相机绘制
/// （bevy_ui 选择「指向主窗口的 order 最高相机」），行为不变。
fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Lunex UiCamera"),
        Camera2d,
        Camera {
            // 不清屏：让 3D 画面透过来，本相机只画 2D UI 层
            clear_color: ClearColorConfig::None,
            // 高于 3D 主相机（默认 order 0），渲染在场景之上
            order: 1,
            ..default()
        },
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
    ));
}

/// 最小界面：顶部标题横幅 +（env 门控）B1 积木面板。
/// 后续 B 阶段逐 Tab 迁移时，egui 面板仍并行保留（feature 切换）。
fn spawn_hud_root(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    theme: Res<LunexTheme>,
    library: Res<BlockLibrary>,
    blueprint: Res<Blueprint>,
    blueprint_library: Res<BlueprintLibrary>,
    challenge: Res<Challenge>,
    challenge_library: Res<ChallengeLibrary>,
    collection: Res<Collection>,
) {
    let font = FontSource::Handle(asset_server.load("fonts/NotoSansSC-subset.otf"));
    // lunex 只重建 Mesh2d 几何，材质需自行提供（UiColor 系统负责着色）
    let banner_material = materials.add(ColorMaterial::from(theme.banner_bg));

    commands
        .spawn((
            Name::new("Lunex HUD Root"),
            UiLayoutRoot::new_2d(),
            UiFetchFromCamera::<0>,
        ))
        .with_children(|ui| {
            ui.spawn((
                Name::new("Title Banner"),
                // 顶部居中横幅：pos 为锚点位置，size 为相对父节点（视口）比例
                UiLayout::window()
                    .pos((Rl(50.0), Rh(6.0)))
                    .size((Rl(36.0), Rh(6.5)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                UiColor::new(vec![(UiBase::id(), theme.banner_bg)]),
                UiMeshPlane2d,
                MeshMaterial2d(banner_material.clone()),
            ))
            .with_children(|banner| {
                banner.spawn((
                    Name::new("Title Text"),
                    Text2d::new("黄鹤楼 · 筑梦江城"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(44.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(55.0)),
                    UiColor::new(vec![(UiBase::id(), theme.accent)]),
                    UiLayout::window().full().pack(),
                    // 纯展示文本，不参与点击
                    Pickable::IGNORE,
                ));
            });

            // B1-B8 积木面板（D1：egui 已移除，默认启用）
            {
                spawn_palette_nodes(
                    ui,
                    &asset_server,
                    &library,
                    &blueprint,
                    &blueprint_library,
                    &challenge,
                    &challenge_library,
                    &collection,
                    &mut materials,
                    &theme,
                );
            }

            // B7 知识卡片（toast；蓝图模式停顿 >12s 触发，8s 后消失）
            let card_mat = materials.add(ColorMaterial::from(Color::srgba(0.09, 0.13, 0.20, 0.95)));
            ui.spawn((
                Name::new("Knowledge Card"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(14.5)))
                    .size((Rl(44.0), Rh(9.5)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                UiMeshPlane2d,
                MeshMaterial2d(card_mat),
                Pickable::IGNORE,
                Visibility::Hidden,
                KnowledgeCardRoot,
            ))
            .with_children(|card| {
                card.spawn((
                    Name::new("knowledge_name"),
                    Text2d::new(""),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(26.0)),
                    UiColor::new(vec![(UiBase::id(), theme.accent)]),
                    UiLayout::window()
                        .pos((Rl(3.0), Rh(8.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                    KnowledgeCardName,
                ));
                card.spawn((
                    Name::new("knowledge_desc"),
                    Text2d::new(""),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(16.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window()
                        .pos((Rl(3.0), Rh(44.0)))
                        .anchor(Anchor::TOP_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                    KnowledgeCardDesc,
                ));
            });
        });
}

/// A2 验证探针（PHOENIX_TEXT3D_PROBE=1 时生成）：世界空间 `Text3d`
/// （bevy_rich_text3d / cosmic-text）渲染中文。默认不生成 —— A2 已验证通过，
/// C1（匾额）正式实现时复用此处的字体注入 + Mesh3d/材质模式。
#[derive(Component)]
struct Text3dProbe;

fn spawn_text3d_probe(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if std::env::var("PHOENIX_TEXT3D_PROBE").is_err() {
        return;
    }
    commands.spawn((
        Name::new("CJK Text3d Probe"),
        Text3d::new("黄鹤楼 · 筑梦江城"),
        Text3dStyling {
            size: 0.9,
            font: "Noto Sans CJK SC".into(),
            color: bevy::color::Srgba::new(0.95, 0.85, 0.40, 1.0),
            align: bevy_rich_text3d::TextAlign::Center,
            ..default()
        },
        // bevy_rich_text3d 需要实体自带 Mesh3d/Mesh2d + 引用字图集的材质
        Mesh3d::default(),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(bevy_rich_text3d::TextAtlas::DEFAULT_IMAGE.clone()),
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        // 塔前空中悬浮（A2 验证后由 C1 正式实现匾额时移除）
        Transform::from_xyz(0.0, 9.0, 7.0),
        Text3dProbe,
    ));
}

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
                    ui_probe_diagnostic,
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
const PALETTE_VISIBLE: usize = 14;
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
                .pos((Rl(50.0), Rh(3.2)))
                .anchor(Anchor::TOP_CENTER)
                .pack(),
            Pickable::IGNORE,
        ));

        // B2 蓝图模式区：完成度进度条 + 主题下拉
        panel
            .spawn((
                Name::new("Blueprint Section"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(11.5)))
                    .size((Rl(94.0), Rh(19.0)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                Pickable::IGNORE,
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
            });

        // 滚动列表窗口
        panel
            .spawn((
                Name::new("Palette Scroll"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(52.0)))
                    .size((Rl(96.0), Rh(92.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                Pickable::IGNORE,
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
                        font,
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

            // B1 积木面板（默认关闭；与 egui 面板并存阶段用 env 打开验证）
            if std::env::var("PHOENIX_LUNEX_PALETTE").is_ok() {
                spawn_palette_nodes(
                    ui,
                    &asset_server,
                    &library,
                    &blueprint,
                    &blueprint_library,
                    &mut materials,
                    &theme,
                );
            }
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

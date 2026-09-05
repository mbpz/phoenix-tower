//! Building selection, blueprint theme/progress, and opacity controls.
use super::{LunexTheme, TabBlocksRoot};
use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::{select_blueprint, Blueprint, BlueprintGhost, BlueprintLibrary};
use crate::building::placement::{BlockRenderAssets, BlueprintAlpha};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::picking::{pointer::PointerId, Pickable};
use bevy::prelude::*;
use bevy_lunex::prelude::*;
use bevy_lunex::UiSelected;

/// 面板内可见行数（窗口高度按此均分）
pub(super) const PALETTE_VISIBLE: usize = 10;
/// 单行高度 = 100% / 可见行数
pub(super) const ROW_H_PCT: f32 = 100.0 / PALETTE_VISIBLE as f32;

/// 积木面板行标记（存积木索引）
#[derive(Component)]
pub struct PaletteRow(pub usize);

/// 面板滚动偏移（行单位，0 = 顶部）
#[derive(Resource, Default)]
pub struct PaletteScroll(pub f32);

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
pub(super) fn palette_sync_selection(
    library: Res<BlockLibrary>,
    mut rows: Query<(&PaletteRow, &mut UiSelected)>,
) {
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
pub(super) fn palette_scroll_system(
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
        if let Some(bevy_lunex::UiLayoutType::Window(w)) = layout.layouts.get_mut(&UiBase::id()) {
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
pub(super) fn b2_progress_sync(
    blueprint: Res<Blueprint>,
    mut fills: Query<&mut UiLayout, With<ProgressFill>>,
    mut texts: Query<&mut Text2d, With<ProgressText>>,
) {
    if !blueprint.is_changed() {
        return;
    }
    let pct = (blueprint.completion.clamp(0.0, 1.0) * 100.0).round();
    for mut fill in &mut fills {
        if let Some(bevy_lunex::UiLayoutType::Window(w)) = fill.layouts.get_mut(&UiBase::id()) {
            w.size = (Rl(pct), Rl(82.0)).into();
        }
    }
    for mut t in &mut texts {
        t.0 = format!("{pct:.0}%");
    }
}

/// 主题按钮点击 → 开合下拉
fn theme_button_click(trigger: On<Pointer<Click>>, mut menu: ResMut<ThemeMenuOpen>) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    menu.0 = !menu.0;
}

/// 下拉开合状态 → 行可见性
pub(super) fn b2_theme_menu_sync(
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
pub(super) fn b2_theme_name_sync(
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
pub(super) fn opacity_from_hit(
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
pub(super) fn b8_opacity_sync(
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
pub(super) fn b8_opacity_apply(
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

pub(super) fn spawn_blueprint_section(
    panel: &mut ChildSpawnerCommands,
    font: &FontSource,
    blueprint: &Blueprint,
    blueprint_library: &BlueprintLibrary,
    materials: &mut Assets<ColorMaterial>,
    theme: &LunexTheme,
) {
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
                Text2d::new(format!(
                    "{:.0}%",
                    blueprint.completion.clamp(0.0, 1.0) * 100.0
                )),
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
                    // 下拉行深度 +2（> 列表行 z=3），打开时浮于列表之上
                    UiDepth::Add(2.0),
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
}

pub(super) fn spawn_palette_list(
    panel: &mut ChildSpawnerCommands,
    font: &FontSource,
    library: &BlockLibrary,
    materials: &mut Assets<ColorMaterial>,
    theme: &LunexTheme,
) {
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
                let row_material = materials.add(ColorMaterial::from(Color::srgba(
                    def.color[0],
                    def.color[1],
                    def.color[2],
                    def.color[3],
                )));
                let swatch_material = materials.add(ColorMaterial::from(Color::srgba(
                    def.color[0],
                    def.color[1],
                    def.color[2],
                    def.color[3],
                )));
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
}

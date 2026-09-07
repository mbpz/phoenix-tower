//! Codex, achievements, and knowledge-card presentation.
use super::palette::{PALETTE_VISIBLE, ROW_H_PCT};
use super::set_hover;
use super::LunexTheme;
use crate::building::block_defs::BlockLibrary;
use crate::building::collection::{achievement_defs, Collection, KnowledgeHints};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::picking::{pointer::PointerId, Pickable};
use bevy::prelude::*;
use bevy_lunex::prelude::*;

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
pub(super) fn b6_achievement_sync(
    collection: Res<Collection>,
    theme: Res<LunexTheme>,
    mut names: Query<(&AchievementNameText, &mut Text2d, &mut UiColor)>,
) {
    if !collection.is_changed() && !theme.is_changed() {
        return;
    }
    for (row, t, mut color) in &mut names {
        let (id, name, desc) = achievement_defs()[row.0];
        let unlocked = collection.achievements.contains(id);
        t.map_unchanged(|t| &mut t.0).set_if_neq(format!(
            "{}  {}\n{}",
            name,
            if unlocked { "✓" } else { "🔒" },
            desc
        ));
        color.set_if_neq(UiColor::new(vec![(
            UiBase::id(),
            if unlocked {
                theme.accent
            } else {
                theme.text_main
            },
        )]));
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

type KnowledgeCardTextParams<'w, 's> = ParamSet<
    'w,
    's,
    (
        Query<'static, 'static, &'static mut Text2d, With<KnowledgeCardName>>,
        Query<'static, 'static, &'static mut Text2d, With<KnowledgeCardDesc>>,
    ),
>;

/// 知识卡片同步：KnowledgeHints.card 变化 → 显隐 + 文本
pub(super) fn b7_knowledge_sync(
    hints: Res<KnowledgeHints>,
    mut cards: Query<&mut Visibility, With<KnowledgeCardRoot>>,
    mut texts: KnowledgeCardTextParams<'_, '_>,
) {
    if !hints.is_changed() {
        return;
    }
    for mut v in &mut cards {
        *v = if hints.card.is_some() {
            Visibility::Inherited
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
// B5：图鉴 Tab（35 条目解锁状态 + 文化描述）
// ======================================================================

/// 图鉴内容根标记
#[derive(Component)]
pub struct TabCodexRoot;

/// 图鉴行（存积木索引）
#[derive(Component)]
pub struct CodexRow(pub usize);

/// 图鉴行文本（独立子节点；存积木索引）
#[derive(Component)]
pub struct CodexNameText(pub usize);

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
pub(super) fn b5_codex_detail_sync(
    selected: Res<CodexSelected>,
    collection: Res<Collection>,
    library: Res<BlockLibrary>,
    mut texts: Query<&mut Text2d, With<CodexDetailText>>,
) {
    if !selected.is_changed() && !collection.is_changed() && !library.is_changed() {
        return;
    }
    let txt = match selected.0 {
        Some(i) if i < library.defs.len() => {
            let def = &library.defs[i];
            let unlocked = collection.codex.contains(&def.id);
            format!(
                "{}  {}\n\n{}",
                def.name,
                if unlocked {
                    "✓ 已解锁"
                } else {
                    "🔒 未解锁"
                },
                def.description
            )
        }
        _ => "点击条目查看文化描述".to_string(),
    };
    for t in &mut texts {
        t.map_unchanged(|t| &mut t.0).set_if_neq(txt.clone());
    }
}

/// 图鉴列表滚动（同积木面板模式：悬停于行上滚轮生效；窗口外行隐藏）
#[allow(clippy::type_complexity)]
pub(super) fn codex_scroll_system(
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
        if let Some(bevy_lunex::UiLayoutType::Window(w)) = layout.layouts.get_mut(&UiBase::id()) {
            w.pos = (Rl(50.0), Rl(y)).into();
        }
        *vis = if y < 0.0 || y > 100.0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// 图鉴行解锁状态同步：Collection 变化 → 行名/状态文本与颜色
pub(super) fn b5_codex_status_sync(
    collection: Res<Collection>,
    library: Res<BlockLibrary>,
    theme: Res<LunexTheme>,
    mut names: Query<(&CodexNameText, &mut Text2d, &mut UiColor)>,
) {
    if !collection.is_changed() && !library.is_changed() && !theme.is_changed() {
        return;
    }
    for (row, t, mut color) in &mut names {
        if row.0 >= library.defs.len() {
            continue;
        }
        let def = &library.defs[row.0];
        let unlocked = collection.codex.contains(&def.id);
        let label = if unlocked {
            format!("{}  ✓", def.name)
        } else {
            format!("{}  🔒", def.name)
        };
        // Collection is also ticked when no unlock changes. Do not invalidate
        // retained text/layout (including hidden tabs) for identical display values.
        t.map_unchanged(|t| &mut t.0).set_if_neq(label);
        color.set_if_neq(UiColor::new(vec![(
            UiBase::id(),
            if unlocked {
                theme.text_main
            } else {
                theme.text_dim
            },
        )]));
    }
}

pub(super) fn spawn_collection_tabs(
    panel: &mut ChildSpawnerCommands,
    font: &FontSource,
    library: &BlockLibrary,
    collection: &Collection,
    materials: &mut Assets<ColorMaterial>,
    theme: &LunexTheme,
) {
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
                UiTextSize::from(Ab(20.0)),
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
                    UiTextSize::from(Ab(15.0)),
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
                            (UiBase::id(), theme.row_base),
                            (UiHover::id(), theme.row_hover),
                        ]),
                        UiHover::new().instant(true),
                        UiMeshPlane2d,
                        MeshMaterial2d(row_mat),
                        CodexRow(i),
                    ))
                    .observe(set_hover::<Pointer<Over>, true>)
                    .observe(set_hover::<Pointer<Out>, false>)
                    .observe(codex_row_click)
                    .with_children(|row| {
                        // 文本必须为独立子节点：文本缩放作用于本实体 Transform，
                        // 与 mesh 同实体会把行放大成整屏色块（B 修复）
                        row.spawn((
                            Name::new("codex_name"),
                            Text2d::new(label),
                            TextFont {
                                font: font.clone(),
                                font_size: FontSize::Px(18.0),
                                ..default()
                            },
                            UiTextSize::from(Ab(18.0)),
                            UiColor::new(vec![(
                                UiBase::id(),
                                if unlocked {
                                    theme.text_main
                                } else {
                                    theme.text_dim
                                },
                            )]),
                            UiLayout::window()
                                .pos((Rl(6.0), Rl(50.0)))
                                .anchor(Anchor::CENTER_LEFT)
                                .pack(),
                            Pickable::IGNORE,
                            CodexNameText(i),
                        ));
                    });
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
                UiTextSize::from(Ab(20.0)),
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
                        UiTextSize::from(Ab(16.0)),
                        UiColor::new(vec![(
                            UiBase::id(),
                            if unlocked {
                                theme.accent
                            } else {
                                theme.text_main
                            },
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
}

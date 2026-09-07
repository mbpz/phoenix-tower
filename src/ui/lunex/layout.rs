//! HUD layout, theme, camera, and tab navigation. Feature sections retain their spawn order.
use super::challenges::spawn_challenge_section;
use super::collection::spawn_collection_tabs;
use super::palette::{spawn_blueprint_section, spawn_palette_list};
use super::saves::spawn_saves_tab;
use super::set_hover;
use super::{
    KnowledgeCardDesc, KnowledgeCardName, KnowledgeCardRoot, TabAchievementsRoot, TabCodexRoot,
};
use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::{Blueprint, BlueprintLibrary};
use crate::building::challenge::{Challenge, ChallengeLibrary};
use crate::building::collection::Collection;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy_lunex::prelude::*;
use bevy_lunex::UiSelected;

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
    mut path: ResMut<super::PathInput>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let Ok(btn) = buttons.get(trigger.event_target()) else {
        return;
    };
    tab.0 = btn.0;
    if btn.0 != LunexTabId::Saves {
        path.focused = false;
    }
}

type TabContentQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Option<&'static TabBlocksRoot>,
        Option<&'static TabSavesRoot>,
        Option<&'static TabCodexRoot>,
        Option<&'static TabAchievementsRoot>,
    ),
    Or<(
        With<TabBlocksRoot>,
        With<TabSavesRoot>,
        With<TabCodexRoot>,
        With<TabAchievementsRoot>,
    )>,
>;

/// Tab 切换 → 各内容根可见性 + Tab 按钮高亮（UiSelected）
pub(super) fn lunex_tab_sync(
    tab: Res<LunexTab>,
    mut contents: TabContentQuery<'_, '_>,
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
            Visibility::Inherited
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
            panel
                .spawn((
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
                .observe(set_hover::<Pointer<Over>, true>)
                .observe(set_hover::<Pointer<Out>, false>)
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
                        UiTextSize::from(Ab(18.0)),
                        UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                        super::typography::centered_label_layout(),
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
            UiTextSize::from(Ab(26.0)),
            UiColor::new(vec![(UiBase::id(), theme.accent)]),
            UiLayout::window()
                .pos((Rl(50.0), Rh(10.5)))
                .anchor(Anchor::TOP_CENTER)
                .pack(),
            Pickable::IGNORE,
            TabBlocksRoot,
        ));

        spawn_blueprint_section(panel, &font, blueprint, blueprint_library, materials, theme);

        spawn_challenge_section(panel, &font, challenge, challenge_library, materials, theme);

        spawn_palette_list(panel, &font, library, materials, theme);

        spawn_saves_tab(panel, &font, materials, theme);

        spawn_collection_tabs(panel, &font, library, collection, materials, theme);
    });
}

/// 2D UI 相机：叠加在 3D 主相机之上（order 更高）、透明清屏，
/// 作为 lunex 布局的尺寸来源（`UiSourceCamera::<0>`）。
///
/// 说明：bevy_ui 的 HUD 提示（hud.rs）会改由这台相机绘制
/// （bevy_ui 选择「指向主窗口的 order 最高相机」），行为不变。
pub(super) fn spawn_ui_camera(mut commands: Commands) {
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
        // MSAA Off：2D UI 无需抗锯齿；与 3D 相机共享窗口目标时，
        // MSAA 跨相机 load/resolve 在 Metal 上会闪烁（见 diagnose 记录）
        Msaa::Off,
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
    ));
}

/// 最小界面：顶部标题横幅 +（env 门控）B1 积木面板。
/// 后续 B 阶段逐 Tab 迁移时，egui 面板仍并行保留（feature 切换）。
pub(super) fn spawn_hud_root(
    riverside: Option<Res<crate::riverside::RiversideMode>>,
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
                    Text2d::new(if riverside.as_ref().is_some_and(|m| m.0) {
                        "江岸亭 · 筑梦庭院"
                    } else {
                        "黄鹤楼 · 筑梦江城"
                    }),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(24.0),
                        ..default()
                    },
                    UiTextSize::from(Ab(24.0)),
                    UiColor::new(vec![(UiBase::id(), theme.accent)]),
                    super::typography::centered_label_layout(),
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
                    UiTextSize::from(Ab(20.0)),
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
                    UiTextSize::from(Ab(15.0)),
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

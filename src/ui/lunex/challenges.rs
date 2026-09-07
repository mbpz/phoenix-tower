//! Challenge selection/start controls and state-dependent labels.
use super::set_hover;
use super::{LunexTheme, TabBlocksRoot};
use crate::building::blueprint::{Blueprint, BlueprintLibrary};
use crate::building::challenge::{
    select_challenge, start_challenge, Challenge, ChallengeLibrary, ChallengeState,
};
use crate::building::placement::{PlacedBlock, PlacedBlocks};
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy_lunex::prelude::*;

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
pub(super) fn b3_challenge_menu_sync(
    menu: Res<ChallengeMenuOpen>,
    mut rows: Query<&mut Visibility, With<ChallengeMenuRow>>,
) {
    if !menu.is_changed() {
        return;
    }
    for mut v in &mut rows {
        *v = if menu.0 {
            Visibility::Inherited
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

type ChallengeTextParams<'w, 's> = ParamSet<
    'w,
    's,
    (
        Query<'static, 'static, &'static mut Text2d, With<ChallengeStatusText>>,
        Query<'static, 'static, &'static mut Text2d, With<ChallengeButtonText>>,
    ),
>;

/// 挑战状态/时间/材料变化 → 状态行 + 按钮文本（进行中每帧 tick，文本更新开销可忽略）
pub(super) fn b3_challenge_sync(challenge: Res<Challenge>, mut texts: ChallengeTextParams<'_, '_>) {
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
    for t in texts.p0() {
        t.map_unchanged(|t| &mut t.0)
            .set_if_neq(status_line.clone());
    }
    for t in texts.p1() {
        t.map_unchanged(|t| &mut t.0)
            .set_if_neq(button_label.clone());
    }
}

/// 挑战库变化（下拉/其他入口）→ 名称文本 + 收起菜单
pub(super) fn b3_challenge_name_sync(
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

pub(super) fn spawn_challenge_section(
    panel: &mut ChildSpawnerCommands,
    font: &FontSource,
    challenge: &Challenge,
    challenge_library: &ChallengeLibrary,
    materials: &mut Assets<ColorMaterial>,
    theme: &LunexTheme,
) {
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
            .observe(set_hover::<Pointer<Over>, true>)
            .observe(set_hover::<Pointer<Out>, false>)
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
                    UiTextSize::from(Ab(15.0)),
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
            .observe(set_hover::<Pointer<Over>, true>)
            .observe(set_hover::<Pointer<Out>, false>)
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
                    UiTextSize::from(Ab(15.0)),
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
                UiTextSize::from(Ab(14.0)),
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
                    // 下拉行深度 +2（> 列表行 z=3），打开时浮于列表之上
                    UiDepth::Add(2.0),
                    UiMeshPlane2d,
                    MeshMaterial2d(row_material),
                    Visibility::Hidden,
                    ChallengeMenuRow(i),
                ))
                .observe(set_hover::<Pointer<Over>, true>)
                .observe(set_hover::<Pointer<Out>, false>)
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
                        UiTextSize::from(Ab(14.0)),
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
}

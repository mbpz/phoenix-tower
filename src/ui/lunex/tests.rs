//! Headless behavior locks for the Lunex module split; no renderer or window required.
use super::*;
use crate::building::block_defs::{load_block_library, BlockLibrary};
use crate::building::blueprint::{load_blueprint, Blueprint};
use crate::building::challenge::{load_challenge, Challenge, ChallengeState};
use crate::building::collection::{KnowledgeCard, KnowledgeHints};
use crate::building::placement::BlueprintAlpha;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy_lunex::UiSelected;

#[test]
fn tabs_show_only_the_selected_content_and_highlight() {
    let mut app = App::new();
    app.init_resource::<LunexTab>()
        .add_systems(Update, lunex_tab_sync);
    let roots = [
        app.world_mut()
            .spawn((TabBlocksRoot, Visibility::Hidden))
            .id(),
        app.world_mut()
            .spawn((TabCodexRoot, Visibility::Hidden))
            .id(),
        app.world_mut()
            .spawn((TabAchievementsRoot, Visibility::Hidden))
            .id(),
        app.world_mut()
            .spawn((TabSavesRoot, Visibility::Hidden))
            .id(),
    ];
    let tabs = [
        LunexTabId::Blocks,
        LunexTabId::Codex,
        LunexTabId::Achievements,
        LunexTabId::Saves,
    ];
    let buttons = tabs.map(|tab| {
        app.world_mut()
            .spawn((TabButton(tab), UiSelected(0.0)))
            .id()
    });
    for (selected, tab) in tabs.into_iter().enumerate() {
        app.world_mut().resource_mut::<LunexTab>().0 = tab;
        app.update();
        for i in 0..roots.len() {
            assert_eq!(
                *app.world().get::<Visibility>(roots[i]).unwrap(),
                if i == selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                }
            );
            assert_eq!(
                app.world().get::<UiSelected>(buttons[i]).unwrap().0,
                if i == selected { 1.0 } else { 0.0 }
            );
        }
    }
}

#[test]
fn palette_highlight_follows_library_selection() {
    let mut app = App::new();
    app.insert_resource(load_block_library())
        .add_systems(Update, palette_sync_selection);
    let rows = [0, 1].map(|i| app.world_mut().spawn((PaletteRow(i), UiSelected(0.0))).id());
    for selected in [0, 1] {
        app.world_mut().resource_mut::<BlockLibrary>().current = selected;
        app.update();
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(
                app.world().get::<UiSelected>(*row).unwrap().0,
                if i == selected { 1.0 } else { 0.0 }
            );
        }
    }
}

#[test]
fn scrolling_initially_hides_overflow_and_ignores_unhovered_wheel() {
    let mut app = App::new();
    app.insert_resource(load_block_library())
        .init_resource::<PaletteScroll>()
        .init_resource::<CodexScroll>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<bevy::picking::hover::HoverMap>()
        .add_systems(Update, (palette_scroll_system, codex_scroll_system));
    let rows = [0, 9, 10].map(|i| {
        (
            app.world_mut()
                .spawn((
                    PaletteRow(i),
                    UiLayout::window().pack(),
                    Visibility::Visible,
                ))
                .id(),
            app.world_mut()
                .spawn((CodexRow(i), UiLayout::window().pack(), Visibility::Visible))
                .id(),
        )
    });
    app.update();
    for (i, (palette, codex)) in rows.into_iter().enumerate() {
        let expected = if i == 2 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        assert_eq!(*app.world().get::<Visibility>(palette).unwrap(), expected);
        assert_eq!(*app.world().get::<Visibility>(codex).unwrap(), expected);
    }
    app.world_mut()
        .resource_mut::<AccumulatedMouseScroll>()
        .delta
        .y = -10.0;
    app.update();
    assert_eq!(app.world().resource::<PaletteScroll>().0, 0.0);
    assert_eq!(app.world().resource::<CodexScroll>().0, 0.0);
}

#[test]
fn progress_clamps_and_rounds_both_label_and_fill() {
    let mut app = App::new();
    app.insert_resource(load_blueprint())
        .add_systems(Update, b2_progress_sync);
    let text = app.world_mut().spawn((ProgressText, Text2d::new(""))).id();
    let fill = app
        .world_mut()
        .spawn((ProgressFill, UiLayout::window().pack()))
        .id();
    for (completion, pct, label) in [
        (-0.2, 0.0, "0%"),
        (0.426, 43.0, "43%"),
        (1.2, 100.0, "100%"),
    ] {
        app.world_mut().resource_mut::<Blueprint>().completion = completion;
        app.update();
        assert_eq!(app.world().get::<Text2d>(text).unwrap().0, label);
        let layout = app.world().get::<UiLayout>(fill).unwrap();
        let bevy_lunex::UiLayoutType::Window(window) = &layout.layouts[&UiBase::id()] else {
            panic!("progress fill must remain a window layout");
        };
        assert_eq!(window.size, (Rl(pct), Rl(82.0)).into());
    }
}

#[test]
fn opacity_hit_mapping_clamps_to_track_and_ignores_unknown_target() {
    use bevy::ecs::system::SystemState;
    let mut world = World::new();
    let track = world
        .spawn((
            OpacitySlider,
            GlobalTransform::from_translation(Vec3::new(100.0, 0.0, 0.0)),
            Dimension(Vec2::new(200.0, 20.0)),
        ))
        .id();
    let other = world.spawn_empty().id();
    let mut state: SystemState<Query<(&GlobalTransform, &Dimension), With<OpacitySlider>>> =
        SystemState::new(&mut world);
    let tracks = state.get(&world).unwrap();
    let mut alpha = BlueprintAlpha::default();
    for (hit, expected) in [
        (-20.0, 0.1),
        (0.0, 0.1),
        (100.0, 0.45),
        (200.0, 0.8),
        (250.0, 0.8),
    ] {
        opacity_from_hit(hit, &tracks, track, &mut alpha);
        assert!((alpha.value - expected).abs() < 1e-6);
    }
    opacity_from_hit(0.0, &tracks, other, &mut alpha);
    assert!((alpha.value - 0.8).abs() < 1e-6);
}

#[test]
fn knowledge_card_updates_text_and_hides_when_cleared() {
    let mut app = App::new();
    app.init_resource::<KnowledgeHints>()
        .add_systems(Update, b7_knowledge_sync);
    let root = app
        .world_mut()
        .spawn((KnowledgeCardRoot, Visibility::Visible))
        .id();
    let name = app
        .world_mut()
        .spawn((KnowledgeCardName, Text2d::new("")))
        .id();
    let desc = app
        .world_mut()
        .spawn((KnowledgeCardDesc, Text2d::new("")))
        .id();
    app.update();
    assert_eq!(
        *app.world().get::<Visibility>(root).unwrap(),
        Visibility::Hidden
    );
    app.world_mut().resource_mut::<KnowledgeHints>().card = Some(KnowledgeCard {
        name: "斗拱".into(),
        desc: "承托屋檐".into(),
    });
    app.update();
    assert_eq!(
        *app.world().get::<Visibility>(root).unwrap(),
        Visibility::Visible
    );
    assert_eq!(app.world().get::<Text2d>(name).unwrap().0, "📖 斗拱");
    assert_eq!(app.world().get::<Text2d>(desc).unwrap().0, "承托屋檐");
    app.world_mut().resource_mut::<KnowledgeHints>().card = None;
    app.update();
    assert_eq!(
        *app.world().get::<Visibility>(root).unwrap(),
        Visibility::Hidden
    );
    // Clearing the card hides it without clearing the old label.
    assert_eq!(app.world().get::<Text2d>(name).unwrap().0, "📖 斗拱");
}

#[test]
fn challenge_labels_follow_all_four_states() {
    let mut app = App::new();
    app.insert_resource(load_challenge())
        .add_systems(Update, b3_challenge_sync);
    let status = app
        .world_mut()
        .spawn((ChallengeStatusText, Text2d::new("")))
        .id();
    let button = app
        .world_mut()
        .spawn((ChallengeButtonText, Text2d::new("")))
        .id();
    let (used, total) = app.world().resource::<Challenge>().quota_used_total();
    for (state, expected_status, expected_button) in [
        (ChallengeState::Idle, "未开始".to_string(), "开始"),
        (
            ChallengeState::Active,
            format!("⏱ 13s  🧱 {used}/{total}"),
            "进行中",
        ),
        (
            ChallengeState::Won,
            "★ ★★（再来一局）".to_string(),
            "再来一局",
        ),
        (ChallengeState::Failed, "⏱ 失败（重试）".to_string(), "重试"),
    ] {
        {
            let mut challenge = app.world_mut().resource_mut::<Challenge>();
            challenge.state = state;
            challenge.time_left = 12.6;
            challenge.stars = 2;
        }
        app.update();
        assert_eq!(
            app.world().get::<Text2d>(status).unwrap().0,
            expected_status
        );
        assert_eq!(
            app.world().get::<Text2d>(button).unwrap().0,
            expected_button
        );
    }
}

#[test]
fn path_input_mirrors_placeholder_value_and_focus() {
    let mut app = App::new();
    app.init_resource::<PathInput>()
        .add_message::<KeyboardInput>()
        .add_systems(Update, path_input_system);
    let text = app.world_mut().spawn((PathInputText, Text2d::new(""))).id();
    let input_box = app.world_mut().spawn((PathInputBox, UiSelected(1.0))).id();
    app.update();
    assert_eq!(app.world().get::<Text2d>(text).unwrap().0, "输入路径…");
    assert_eq!(app.world().get::<UiSelected>(input_box).unwrap().0, 0.0);
    {
        let mut input = app.world_mut().resource_mut::<PathInput>();
        input.value = "  /tmp/黄鹤楼.ptw  ".into();
        input.focused = true;
    }
    app.update();
    assert_eq!(
        app.world().get::<Text2d>(text).unwrap().0,
        "  /tmp/黄鹤楼.ptw  "
    );
    assert_eq!(app.world().get::<UiSelected>(input_box).unwrap().0, 1.0);
}

#[derive(Resource, Default)]
struct TextChangeCount(usize);

fn count_changed_labels(texts: Query<(), Changed<Text2d>>, mut count: ResMut<TextChangeCount>) {
    count.0 = texts.iter().count();
}

#[test]
fn idle_path_input_does_not_dirty_text_every_frame() {
    let mut app = App::new();
    app.init_resource::<PathInput>()
        .init_resource::<TextChangeCount>()
        .add_message::<KeyboardInput>()
        .add_systems(Update, (path_input_system, count_changed_labels).chain());
    app.world_mut().spawn((PathInputText, Text2d::new("")));
    app.update();
    app.update();
    assert_eq!(app.world().resource::<TextChangeCount>().0, 0);
    app.world_mut().resource_mut::<PathInput>().value = "建筑.ptw".into();
    app.update();
    assert_eq!(app.world().resource::<TextChangeCount>().0, 1);
}

#[test]
fn challenge_tick_without_display_change_does_not_dirty_labels() {
    let mut challenge = load_challenge();
    challenge.state = ChallengeState::Active;
    challenge.time_left = 100.0;
    let mut app = App::new();
    app.insert_resource(challenge)
        .init_resource::<TextChangeCount>()
        .add_systems(Update, (b3_challenge_sync, count_changed_labels).chain());
    app.world_mut()
        .spawn((ChallengeStatusText, Text2d::new("")));
    app.world_mut()
        .spawn((ChallengeButtonText, Text2d::new("")));
    app.update();
    app.world_mut().resource_mut::<Challenge>().time_left = 99.9;
    app.update();
    assert_eq!(app.world().resource::<TextChangeCount>().0, 0);
    app.world_mut().resource_mut::<Challenge>().time_left = 99.0;
    app.update();
    assert_eq!(app.world().resource::<TextChangeCount>().0, 1);
}

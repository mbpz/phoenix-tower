//! HUD 提示（对应 PRD §3.3 引导与教学的基础文本层）。
//! Phase 1 升级为 bevy_egui 积木面板（BACKLOG B-10）。
//!
//! 中文字体：assets/fonts/NotoSansSC-subset.otf（由 Noto Sans CJK SC 子集化，
//! 仅含游戏实际使用字符，见 docs/BACKLOG B-10）。

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::Blueprint;
use crate::building::challenge::{Challenge, ChallengeState};
use crate::building::placement::PlacedBlocks;
use crate::building::tutorial::Tutorial;
use crate::i18n::{t, Lang, Locale};
use crate::stability::{RemoveMode, StabilityTest, TestState};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HelpVisible>()
            .add_systems(Startup, spawn_hint)
            .add_systems(Update, (toggle_help, update_hint).chain());
    }
}

#[derive(Component)]
struct HintText;

#[derive(Resource, Default)]
struct HelpVisible(bool);

fn toggle_help(
    keys: Res<ButtonInput<KeyCode>>,
    mut help: ResMut<HelpVisible>,
    ownership: Option<Res<crate::ui::input::InputOwnership>>,
) {
    if crate::ui::input::shortcuts_allowed(&keys, ownership.as_deref())
        && keys.just_pressed(KeyCode::F1)
    {
        help.0 = !help.0;
    }
}

fn spawn_hint(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Text::new(""),
        TextLayout::linebreak(bevy::text::LineBreak::AnyCharacter),
        TextFont {
            font: FontSource::Handle(asset_server.load("fonts/NotoSansSC-subset.otf")),
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            // Bound text in logical pixels; never stretch across the play surface.
            bottom: Val::Px(16.0),
            right: Val::Px(16.0),
            width: Val::Px(440.0),
            // Bevy 0.19 re-resolves a measured leaf's percentage max-width
            // against its content width, over-wrapping the height measurement.
            // This root is viewport-relative; Vw resolves to pixels before measure.
            max_width: Val::Vw(50.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.045, 0.065, 0.92)),
        Pickable::IGNORE,
        HintText,
    ));
}

fn update_hint(
    riverside: Option<Res<crate::riverside::RiversideMode>>,
    help: Option<Res<HelpVisible>>,
    mut hint: Query<&mut Text, With<HintText>>,
    library: Res<BlockLibrary>,
    blueprint: Res<Blueprint>,
    tutorial: Res<Tutorial>,
    challenge: Res<Challenge>,
    stack: Res<PlacedBlocks>,
    remove: Res<RemoveMode>,
    stability: Res<StabilityTest>,
    locale: Res<Locale>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    mut fps_display: Local<Option<(f64, f64)>>,
) {
    let def = library.current_def();
    let Ok(text) = hint.single_mut() else {
        return;
    };
    let lang: Lang = locale.lang;
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    // FPS is informational: sample twice per second instead of reshaping the
    // entire CJK hint on every diagnostic update. Gameplay state stays immediate.
    let now = time.elapsed_secs_f64();
    let (sampled_at, sampled_fps) = fps_display.get_or_insert((now, fps));
    if now - *sampled_at >= 0.5 {
        *sampled_at = now;
        *sampled_fps = fps;
    }
    let fps = *sampled_fps;
    let mode = if blueprint.active {
        format!(
            "{}: {}  {} {:.0}%",
            t("蓝图模式", "Blueprint", lang),
            t("引导搭建", "Guided build", lang),
            t("完成度", "done", lang),
            blueprint.completion * 100.0
        )
    } else {
        t("自由模式", "Free Build", lang).to_string()
    };
    let tutorial_line = if riverside.as_ref().is_some_and(|mode| mode.0) && !tutorial.active {
        t(
            "江岸亭：撤销屋顶 → M 开启蓝图 → 点击绿色目标重建；F5 保存",
            "Riverside: undo roof > M for snap guide > click green target; F5 saves",
            lang,
        )
        .to_string()
    } else if tutorial.active {
        let label = match lang {
            Lang::Zh => &tutorial.steps[tutorial.step].label,
            Lang::En => &tutorial.steps[tutorial.step].label_en,
        };
        format!("{label}（{}）", t("N 跳过", "N to skip", lang))
    } else {
        String::new()
    };
    let challenge_line = match challenge.state {
        ChallengeState::Active => {
            let (used, total) = challenge.quota_used_total();
            format!(
                "🏆 {}: {}  ⏱ {:.0}s  🧱 {used}/{total}",
                t("挑战", "Challenge", lang),
                challenge.def.name,
                challenge.time_left
            )
        }
        ChallengeState::Won => format!(
            "🏆 {}: {} ★（C {}）",
            t("挑战完成", "Challenge won", lang),
            "★".repeat(challenge.stars as usize),
            t("再来一局", "again", lang)
        ),
        ChallengeState::Failed => format!(
            "⏱ {}（C {}）",
            t("挑战失败", "Failed", lang),
            t("重试", "retry", lang)
        ),
        ChallengeState::Idle => String::new(),
    };
    let stability_line = match stability.state {
        TestState::Running => format!(
            "🏗 {}… {:.0}s",
            t("重力测试中", "Gravity test", lang),
            stability.timer
        ),
        TestState::Done => format!(
            "🏗 {}: {:.0}% → {} ★（G {}）",
            t("测试完成 存活", "Survival", lang),
            stability.survival * 100.0,
            "★".repeat(stability.stars as usize),
            t("复原", "restore", lang)
        ),
        TestState::Idle => String::new(),
    };
    let tool_line = if remove.active {
        t(
            "🔧 拆除模式：点击移除积木（X 退出）",
            "🔧 Remove mode: click to remove (X)",
            lang,
        )
        .to_string()
    } else {
        String::new()
    };
    let keys_line = match lang {
        Lang::Zh => {
            "左键:放置  左拖:旋转  右拖:平移  滚轮:缩放  Home:全景\n\
         撤销:Backspace / Cmd+Z / Ctrl+Z\n\
         重做:Cmd+Shift+Z / Ctrl+Y（20 步）\n\
         1-9:选积木  Q/E:切换  R:旋转90°  X:拆除  M:蓝图/自由\n\
         L:中/英  K:知识提示  T:昼夜  G:重力测试  C:挑战\n\
         F2:截图  F5:保存  F6:JSON  F7:分享  F8:导入  F9:槽位"
        }
        Lang::En => {
            "LMB:place  LMB-drag:orbit  RMB-drag:pan  wheel:zoom  Home:overview\n\
         undo:Backspace / Cmd+Z / Ctrl+Z\n\
         redo:Cmd+Shift+Z / Ctrl+Y (20)\n\
         1-9:blocks  Q/E:cycle  R:rotate  X:remove  M:blueprint/free\n\
         L:zh/en  K:knowledge  T:day/night  G:physics  C:challenge\n\
         F2:shot  F5:save  F6:JSON  F7:share  F8:import  F9:slot"
        }
    };
    let compact_keys = t(
        "点击放置 · 拖动旋转 · 滚轮缩放 · F1 全部操作",
        "Click: place · Drag: orbit · Wheel: zoom · F1: help",
        lang,
    );
    let mut lines = vec![
        if help.is_some_and(|h| h.0) {
            keys_line.to_string()
        } else {
            compact_keys.to_string()
        },
        format!(
            "{}: {} · {} {}° · {}",
            t("积木", "Block", lang),
            def.name,
            t("旋转", "Rotation", lang),
            library.rotation as u32 * 90,
            mode
        ),
    ];
    for line in [challenge_line, stability_line, tool_line, tutorial_line] {
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.push(format!(
        "FPS {fps:.0} · {} {}",
        stack.records.len(),
        t("块积木", "blocks", lang)
    ));
    let value = lines.join("\n");
    text.map_unchanged(|t| &mut t.0).set_if_neq(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::{
        block_defs::load_block_library, blueprint::load_blueprint, challenge::load_challenge,
    };
    use bevy::diagnostic::{Diagnostic, DiagnosticMeasurement};
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct TextChanges(usize);

    fn count_changes(texts: Query<(), Changed<Text>>, mut count: ResMut<TextChanges>) {
        count.0 = texts.iter().count();
    }

    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(load_block_library())
            .insert_resource(load_blueprint())
            .insert_resource(load_challenge())
            .insert_resource(Tutorial {
                active: false,
                step: 0,
                steps: vec![],
            })
            .init_resource::<PlacedBlocks>()
            .init_resource::<RemoveMode>()
            .init_resource::<StabilityTest>()
            .init_resource::<Locale>()
            .init_resource::<DiagnosticsStore>()
            .init_resource::<Time>()
            .init_resource::<TextChanges>()
            .add_systems(Update, (update_hint, count_changes).chain());
        let entity = app.world_mut().spawn((HintText, Text::new(""))).id();
        (app, entity)
    }

    fn fps(app: &mut App, value: f64) {
        let mut diagnostic = Diagnostic::new(FrameTimeDiagnosticsPlugin::FPS);
        diagnostic.add_measurement(DiagnosticMeasurement {
            time: bevy::platform::time::Instant::now(),
            value,
        });
        app.world_mut()
            .resource_mut::<DiagnosticsStore>()
            .add(diagnostic);
    }

    // Exercise Bevy's actual text measure + Taffy layout, not newline counts:
    // a percentage max-width on a measured leaf can reserve invisible wrapped rows.
    #[test]
    fn hint_background_tracks_wrapped_text_at_native_scales() {
        use bevy::asset::AssetPlugin;
        use bevy::ecs::system::{RunSystemOnce, SystemState};
        use bevy::text::{
            load_font_assets_into_font_collection, ComputedTextBlock, FontCx, FontLoader, LayoutCx,
            TextBounds, TextMeasureInfo, TextPipeline,
        };
        use bevy::ui::{ui_surface::UiSurface, widget::TextMeasure, LayoutContext, NodeMeasure};

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset_loader::<FontLoader>()
            .init_resource::<FontCx>();
        app.world_mut().run_system_once(spawn_hint).unwrap();
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<HintText>>()
            .single(app.world())
            .unwrap();
        // Load synchronously so the regression needs neither a window nor IO polling.
        let font = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(
                include_bytes!("../../assets/fonts/NotoSansSC-subset.otf").to_vec(),
            ));
        app.world_mut().get_mut::<TextFont>(entity).unwrap().font = FontSource::Handle(font);
        app.world_mut()
            .run_system_once(load_font_assets_into_font_collection)
            .unwrap();
        let node = app.world().get::<Node>(entity).unwrap().clone();
        let font = app.world().get::<TextFont>(entity).unwrap().clone();
        let text_layout = *app.world().get::<TextLayout>(entity).unwrap();
        let fonts = app.world_mut().remove_resource::<Assets<Font>>().unwrap();
        let mut font_cx = app.world_mut().remove_resource::<FontCx>().unwrap();
        let mut pipeline = TextPipeline::default();
        let mut layout_cx = LayoutCx::default();
        let mut surface = UiSurface::default();
        let mut buffers = SystemState::<Query<&mut ComputedTextBlock>>::new(app.world_mut());

        for scale in [1.0, 2.0] {
            for viewport_width in [1280.0, 640.0] {
                let viewport = Vec2::new(viewport_width, 752.0);
                let physical_size = viewport * scale;
                let mut previous_height = f32::INFINITY;
                // Long -> short covers hiding tutorial/help without a stale tall panel.
                for line_count in [5, 3] {
                    let text = vec!["积木".repeat(12); line_count].join("\n");
                    app.world_mut().get_mut::<Text>(entity).unwrap().0 = text.clone();
                    let measure = pipeline
                        .create_text_measure(
                            entity,
                            &fonts,
                            std::iter::once((
                                entity,
                                0,
                                text.as_str(),
                                &font,
                                Color::WHITE,
                                default(),
                                default(),
                            )),
                            scale,
                            &text_layout,
                            &mut app
                                .world_mut()
                                .get_mut::<ComputedTextBlock>(entity)
                                .unwrap(),
                            &mut font_cx,
                            &mut layout_cx,
                            viewport,
                            16.0,
                        )
                        .unwrap();
                    let mut expected_measure = TextMeasureInfo {
                        min: measure.min,
                        max: measure.max,
                        entity,
                    };
                    surface.upsert_node(
                        &LayoutContext {
                            scale_factor: scale,
                            physical_size,
                        },
                        entity,
                        &node,
                        Some(NodeMeasure::Text(TextMeasure { info: measure })),
                    );
                    surface.compute_layout(
                        entity,
                        physical_size.as_uvec2(),
                        &mut buffers.get_mut(app.world_mut()).unwrap(),
                        &mut font_cx,
                    );
                    let layout = surface.get_layout(entity, true).unwrap().0;
                    let text_size = expected_measure.compute_size(
                        TextBounds::new_horizontal(layout.content_box_width()),
                        &mut app
                            .world_mut()
                            .get_mut::<ComputedTextBlock>(entity)
                            .unwrap(),
                        &mut font_cx,
                    );
                    assert_eq!(
                        layout.size.width / scale,
                        440.0_f32.min(viewport_width * 0.5)
                    );
                    assert!(text_size.y > 0.0);
                    assert!(
                        (layout.size.height - text_size.y - 24.0 * scale).abs() <= 1.0,
                        "scale={scale}, viewport={viewport_width}, lines={line_count}: \
                         panel={}, text={}",
                        layout.size.height,
                        text_size.y,
                    );
                    assert!(layout.size.height < previous_height);
                    previous_height = layout.size.height;
                }
            }
        }
    }

    #[test]
    fn expanded_help_lists_readable_mac_and_fallback_history_keys() {
        for lang in [Lang::Zh, Lang::En] {
            let (mut app, entity) = app();
            app.insert_resource(HelpVisible(true));
            app.world_mut().resource_mut::<Locale>().lang = lang;
            app.update();
            let text = &app.world().get::<Text>(entity).unwrap().0;
            for key in ["Backspace", "Cmd+Z", "Cmd+Shift+Z", "Ctrl+Z", "Ctrl+Y"] {
                assert!(text.contains(key), "Missing {key}: {text}");
            }
            assert!(
                !text.contains(['⌘', '⇧']),
                "Use font-supported key names: {text}"
            );
        }
    }

    #[test]
    fn default_hint_is_compact_and_has_no_blank_rows() {
        let (mut app, entity) = app();
        app.update();
        let text = &app.world().get::<Text>(entity).unwrap().0;
        assert!(text.lines().count() <= 5, "{text}");
        assert!(text.lines().all(|line| !line.trim().is_empty()));
        assert!(text.contains("F1"));
    }

    #[test]
    fn unchanged_hint_does_not_trigger_text_layout() {
        let (mut app, entity) = app();
        app.update();
        let initial = app.world().get::<Text>(entity).unwrap().0.clone();
        app.update();
        assert_eq!(app.world().resource::<TextChanges>().0, 0);
        assert_eq!(app.world().get::<Text>(entity).unwrap().0, initial);
        app.world_mut().resource_mut::<Locale>().lang = Lang::En;
        app.update();
        assert_eq!(app.world().resource::<TextChanges>().0, 1);
        assert!(app.world().get::<Text>(entity).unwrap().0.contains("Block"));
    }

    #[test]
    fn fps_sampling_is_bounded_but_gameplay_feedback_is_immediate() {
        let (mut app, entity) = app();
        fps(&mut app, 60.0);
        app.update();
        assert!(app
            .world()
            .get::<Text>(entity)
            .unwrap()
            .0
            .contains("FPS 60"));
        fps(&mut app, 30.0);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.update();
        assert_eq!(app.world().resource::<TextChanges>().0, 0);
        app.world_mut().resource_mut::<BlockLibrary>().rotation = 1;
        app.update();
        assert_eq!(app.world().resource::<TextChanges>().0, 1);
        assert!(app.world().get::<Text>(entity).unwrap().0.contains("90°"));
        assert!(app
            .world()
            .get::<Text>(entity)
            .unwrap()
            .0
            .contains("FPS 60"));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(400));
        app.update();
        assert!(app
            .world()
            .get::<Text>(entity)
            .unwrap()
            .0
            .contains("FPS 30"));
    }
}

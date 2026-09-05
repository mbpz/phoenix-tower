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
        app.add_systems(Startup, spawn_hint)
            .add_systems(Update, update_hint);
    }
}

#[derive(Component)]
struct HintText;

fn spawn_hint(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font: FontSource::Handle(asset_server.load("fonts/NotoSansSC-subset.otf")),
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            // 右下角：避免与左侧 egui 积木面板重叠
            bottom: Val::Px(12.0),
            right: Val::Px(12.0),
            ..default()
        },
        HintText,
    ));
}

fn update_hint(
    riverside: Option<Res<crate::riverside::RiversideMode>>,
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
            blueprint.def.name,
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
        format!("🎓 {label}\n（{}）", t("N 键跳过教程", "N to skip", lang))
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
            "左键:放置  左拖:旋转  右拖:平移  滚轮:缩放\n\
         撤销:Backspace/Ctrl+Z  重做:Ctrl+Y（20 步）\n\
         1-9:选积木  Q/E:切换  R:旋转90°  X:拆除  M:蓝图/自由\n\
         L:中/英  K:知识提示  T:昼夜  G:重力测试  C:挑战\n\
         F2:截图  F5:保存  F6:JSON  F7:分享  F8:导入  F9:槽位"
        }
        Lang::En => {
            "LMB:place  LMB-drag:orbit  RMB-drag:pan  wheel:zoom\n\
         undo:Backspace/Ctrl+Z  redo:Ctrl+Y (20)\n\
         1-9:blocks  Q/E:cycle  R:rotate  X:remove  M:blueprint/free\n\
         L:zh/en  K:knowledge  T:day/night  G:physics  C:challenge\n\
         F2:shot  F5:save  F6:JSON  F7:share  F8:import  F9:slot"
        }
    };
    let value = format!(
        "{keys_line}\n\
         {mode}\n\
         {block_label}: {name} [{cur}/{total}] ({layer}) {rot_label} {rot}°\n\
         {challenge_line}\n\
         {stability_line}\n\
         {tool_line}\n\
         {tutorial_line}\n\
         FPS {fps:.0} | {blocks_label} {blocks}",
        keys_line = keys_line,
        block_label = t("当前积木", "Block", lang),
        name = def.name,
        cur = library.current + 1,
        total = library.defs.len(),
        layer = def.layer,
        rot_label = t("旋转", "rot", lang),
        rot = library.rotation as u32 * 90,
        mode = mode,
        challenge_line = challenge_line,
        stability_line = stability_line,
        tool_line = tool_line,
        tutorial_line = tutorial_line,
        fps = fps,
        blocks_label = t("积木", "blocks", lang),
        blocks = stack.records.len(),
    );
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

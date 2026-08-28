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
) {
    let def = library.current_def();
    let Ok(mut text) = hint.single_mut() else {
        return;
    };
    let lang: Lang = locale.lang;
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
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
    let tutorial_line = if tutorial.active {
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
         L:中/英  T:昼夜  G:重力测试  C:挑战\n\
         F2:截图  F5:保存  F6:JSON  F7:分享  F8:导入  F9:槽位"
        }
        Lang::En => {
            "LMB:place  LMB-drag:orbit  RMB-drag:pan  wheel:zoom\n\
         undo:Backspace/Ctrl+Z  redo:Ctrl+Y (20)\n\
         1-9:blocks  Q/E:cycle  R:rotate  X:remove  M:blueprint/free\n\
         L:zh/en  T:day/night  G:physics  C:challenge\n\
         F2:shot  F5:save  F6:JSON  F7:share  F8:import  F9:slot"
        }
    };
    text.0 = format!(
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
}

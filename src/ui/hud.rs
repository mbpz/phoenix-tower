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
    diagnostics: Res<DiagnosticsStore>,
) {
    let def = library.current_def();
    let Ok(mut text) = hint.single_mut() else {
        return;
    };
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    let mode = if blueprint.active {
        format!(
            "蓝图模式: {}  完成度 {:.0}%",
            blueprint.def.name,
            blueprint.completion * 100.0
        )
    } else {
        "自由模式".to_string()
    };
    let tutorial_line = if tutorial.active {
        format!("🎓 {}\n（N 键跳过教程）", tutorial.steps[tutorial.step].label)
    } else {
        String::new()
    };
    let challenge_line = match challenge.state {
        ChallengeState::Active => {
            let (used, total) = challenge.quota_used_total();
            format!(
                "🏆 挑战: {}  ⏱ {:.0}s  🧱 {used}/{total}",
                challenge.def.name, challenge.time_left
            )
        }
        ChallengeState::Won => {
            format!("🏆 挑战完成: {} ★（C 再来一局）", "★".repeat(challenge.stars as usize))
        }
        ChallengeState::Failed => "⏱ 挑战失败，按 C 重试".to_string(),
        ChallengeState::Idle => String::new(),
    };
    text.0 = format!(
        "左键:放置  左拖:旋转  右拖:平移  滚轮:缩放\n\
         撤销:Backspace/Ctrl+Z  重做:Ctrl+Y（20 步）\n\
         1-9:选积木  Q/E:切换  R:旋转90°  M:蓝图/自由  T:昼夜\n\
         C:挑战  F5:保存  F6:导出JSON  F9:读取  Esc:退出\n\
         {mode}\n\
         当前积木: {name} [{cur} / {total}]（{layer}） 旋转 {rot}°\n\
         {challenge_line}\n\
         {tutorial_line}\n\
         FPS {fps:.0} | 积木 {blocks}",
        name = def.name,
        cur = library.current + 1,
        total = library.defs.len(),
        layer = def.layer,
        rot = library.rotation as u32 * 90,
        mode = mode,
        challenge_line = challenge_line,
        tutorial_line = tutorial_line,
        fps = fps,
        blocks = stack.records.len(),
    );
}

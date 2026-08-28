//! HUD 提示（对应 PRD §3.3 引导与教学的基础文本层）。
//! Phase 1 升级为 bevy_egui 积木面板（BACKLOG B-10）。

use bevy::prelude::*;

use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::Blueprint;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hint)
            .add_systems(Update, update_hint);
    }
}

#[derive(Component)]
struct HintText;

fn spawn_hint(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        HintText,
    ));
}

fn update_hint(
    mut hint: Query<&mut Text, With<HintText>>,
    library: Res<BlockLibrary>,
    blueprint: Res<Blueprint>,
) {
    let def = library.current_def();
    let Ok(mut text) = hint.single_mut() else {
        return;
    };
    let mode = if blueprint.active {
        format!(
            "蓝图模式: {}  完成度 {:.0}%",
            blueprint.def.name,
            blueprint.completion * 100.0
        )
    } else {
        "自由模式".to_string()
    };
    text.0 = format!(
        "左键:放置  左拖:旋转  右拖:平移  滚轮:缩放  Backspace:撤销\n\
         1-9:选积木  Q/E:切换  M:蓝图/自由  T:昼夜  Esc:退出\n\
         {mode}\n\
         当前积木: {} [{} / {}]（{}）",
        def.name,
        library.current + 1,
        library.defs.len(),
        def.layer
    );
}

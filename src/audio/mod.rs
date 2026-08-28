//! 音频（B-19）：放置音效 / 完成编钟 / 环境江水 + 风铃。
//!
//! 音效资产为程序化生成的 WAV（tools/gen_audio.py，可复现）：
//! - place_wood.wav / place_stone.wav：放置碰撞
//! - bell_chime.wav：蓝图完成编钟
//! - river_loop.wav / wind_chime_loop.wav：环境循环音
//!
//! 对应 PRD §5 音频规范：放置清脆木石碰撞声、完成时悠扬编钟、环境长江水声+风铃声。

use bevy::audio::{AudioPlayer, PlaybackMode, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::building::block_defs::BlockCategory;

/// 音效资产句柄（Startup 加载，全局复用）。
#[derive(Resource)]
pub struct AudioAssets {
    pub place_wood: Handle<AudioSource>,
    pub place_stone: Handle<AudioSource>,
    pub bell_chime: Handle<AudioSource>,
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_audio_assets, spawn_ambience))
            .add_systems(Update, completion_bell_system);
    }
}

fn load_audio_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(AudioAssets {
        place_wood: asset_server.load("audio/place_wood.wav"),
        place_stone: asset_server.load("audio/place_stone.wav"),
        bell_chime: asset_server.load("audio/bell_chime.wav"),
    });
}

/// 环境音：长江水声 + 风铃（低音量循环）。
fn spawn_ambience(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        AudioPlayer::<AudioSource>(asset_server.load("audio/river_loop.wav")),
        PlaybackSettings {
            mode: PlaybackMode::Loop,
            volume: Volume::Linear(0.14),
            ..default()
        },
        Name::new("RiverAmbience"),
    ));
    commands.spawn((
        AudioPlayer::<AudioSource>(asset_server.load("audio/wind_chime_loop.wav")),
        PlaybackSettings {
            mode: PlaybackMode::Loop,
            volume: Volume::Linear(0.05),
            ..default()
        },
        Name::new("WindChimeAmbience"),
    ));
}

/// 放置音效种类：按积木分类选择（纯函数，可测试）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SoundKind {
    /// 木质：结构 / 装饰 / 特殊
    Wood,
    /// 石瓦：基础 / 屋顶
    Stone,
}

pub fn sound_kind_for(category: &BlockCategory) -> SoundKind {
    match category {
        BlockCategory::Base | BlockCategory::Roof => SoundKind::Stone,
        BlockCategory::Structure
        | BlockCategory::Decoration
        | BlockCategory::Special => SoundKind::Wood,
    }
}

/// 播放一次性放置音效（由放置系统在成功放置时调用）。
pub fn play_placement_sound(commands: &mut Commands, audio: &AudioAssets, kind: SoundKind) {
    let handle = match kind {
        SoundKind::Wood => audio.place_wood.clone(),
        SoundKind::Stone => audio.place_stone.clone(),
    };
    commands.spawn((
        AudioPlayer(handle),
        PlaybackSettings {
            mode: PlaybackMode::Despawn,
            volume: Volume::Linear(0.5),
            ..default()
        },
        Name::new("PlacementSfx"),
    ));
}

/// 完成编钟：蓝图完成（≥95%）上升沿触发一次。
fn completion_bell_system(
    blueprint: Res<crate::building::blueprint::Blueprint>,
    audio: Res<AudioAssets>,
    mut commands: Commands,
    mut prev_completed: Local<bool>,
) {
    if blueprint.completed && !*prev_completed {
        commands.spawn((
            AudioPlayer(audio.bell_chime.clone()),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(0.6),
                ..default()
            },
            Name::new("CompletionBell"),
        ));
    }
    *prev_completed = blueprint.completed;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_and_roof_are_stone() {
        assert_eq!(sound_kind_for(&BlockCategory::Base), SoundKind::Stone);
        assert_eq!(sound_kind_for(&BlockCategory::Roof), SoundKind::Stone);
    }

    #[test]
    fn structure_and_decoration_are_wood() {
        assert_eq!(sound_kind_for(&BlockCategory::Structure), SoundKind::Wood);
        assert_eq!(sound_kind_for(&BlockCategory::Decoration), SoundKind::Wood);
        assert_eq!(sound_kind_for(&BlockCategory::Special), SoundKind::Wood);
    }
}

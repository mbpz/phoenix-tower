//! Bevy-Lunex UI（A1：基础集成与最小界面，docs/UI_LUNEX_MIGRATION.md）
//!
//! 参考 Bevypunk（IDEDARY/Bevypunk，Cyberpunk UI 复刻）的 UI 架构：
//! - 独立 `Camera2d`（透明清屏、order 高于 3D 主相机）叠加在场景之上；
//! - 该相机挂 `UiSourceCamera::<0>`，UI 根实体挂 `UiLayoutRoot::new_2d()` +
//!   `UiFetchFromCamera::<0>` 自动同步视口尺寸；
//! - 文本走 Bevy 原生 `Text2d` + `UiTextSize`（lunex 按父节点比例缩放）。
//!
//! 交互（A3）：lunex 0.7 基于 bevy_picking（`Pointer<Click/Over/Out>` 观察者、
//! `Pickable`）。本阶段先验证渲染，交互在 A3 落地。

use bevy::prelude::*;
use bevy_lunex::prelude::*;
use bevy_rich_text3d::LoadFonts;

mod challenges;
mod collection;
mod layout;
mod palette;
mod probes;
mod saves;

// Preserve the existing ui::lunex public component/resource paths. Feature systems
// are only pub(super), so these exports do not widen their visibility.
pub use challenges::*;
pub use collection::*;
pub use layout::*;
pub use palette::*;
use probes::{spawn_text3d_probe, ui_probe_diagnostic, ui_probe_tabs};
pub use saves::*;

pub struct LunexUiPlugin;

impl Plugin for LunexUiPlugin {
    fn build(&self, app: &mut App) {
        // 让 bevy_rich_text3d 的字库包含我们的 CJK 子集字体（A2 验证 + C1 匾额用）。
        // Text3dPlugin 在插件组 cleanup 时读取本资源；init_resource 不覆盖已存在的值，
        // 因此这里先填充再 add_plugins。
        app.init_resource::<LoadFonts>();
        let subset = format!(
            "{}/assets/fonts/NotoSansSC-subset.otf",
            env!("CARGO_MANIFEST_DIR")
        );
        app.world_mut()
            .resource_mut::<LoadFonts>()
            .font_paths
            .push(subset);
        app.init_resource::<LunexTheme>()
            .init_resource::<PaletteScroll>()
            .init_resource::<ThemeMenuOpen>()
            .init_resource::<ChallengeMenuOpen>()
            .init_resource::<LunexTab>()
            .init_resource::<PathInput>()
            .init_resource::<CodexScroll>()
            .init_resource::<CodexSelected>()
            .init_resource::<OpacityDragging>()
            .add_plugins(UiLunexPlugins)
            .add_systems(
                Startup,
                (spawn_ui_camera, spawn_hud_root, spawn_text3d_probe),
            )
            .add_systems(
                Update,
                (
                    palette_scroll_system,
                    palette_sync_selection,
                    b2_progress_sync,
                    b2_theme_menu_sync,
                    b2_theme_name_sync,
                    b3_challenge_sync,
                    b3_challenge_menu_sync,
                    b3_challenge_name_sync,
                ),
            )
            .add_systems(
                Update,
                (
                    lunex_tab_sync,
                    save_list_system,
                    path_input_system,
                    codex_scroll_system,
                    b5_codex_detail_sync,
                    b5_codex_status_sync,
                    b6_achievement_sync,
                    b7_knowledge_sync,
                    b8_opacity_sync,
                    b8_opacity_apply,
                    ui_probe_diagnostic,
                    ui_probe_tabs,
                ),
            );
    }
}

#[cfg(test)]
mod tests;

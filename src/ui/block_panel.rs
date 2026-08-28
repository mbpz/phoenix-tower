//! 积木面板（B-10）：bevy_egui 可视化积木选择 + 完成度进度条。
//!
//! - 左侧面板：15 种积木列表（按积木本色着色），点击选中，当前项高亮
//! - 蓝图模式下显示完成度进度条（ADR-005 加权完成度）
//! - 注入 CJK 子集字体（assets/fonts/NotoSansSC-subset.otf），中文正常显示
//!
//! 依赖 bevy_egui 0.42 / egui 0.36 API：统一 `egui::Panel`（替代旧 SidePanel），
//! 面板在根 viewport Ui 内显示（见 bevy_egui 官方 side_panel 示例）。

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, FontData, FontDefinitions, LayerId, RichText, Ui, UiBuilder};
use bevy_egui::EguiContexts;

use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::Blueprint;

pub struct BlockPanelPlugin;

impl Plugin for BlockPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, block_panel_ui);
    }
}

fn block_panel_ui(
    mut contexts: EguiContexts,
    mut fonts_loaded: Local<bool>,
    mut library: ResMut<BlockLibrary>,
    blueprint: Res<Blueprint>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // 首次运行时注入 CJK 子集字体（egui 默认字体不含中文）
    if !*fonts_loaded {
        let path = format!(
            "{}/assets/fonts/NotoSansSC-subset.otf",
            env!("CARGO_MANIFEST_DIR")
        );
        if let Ok(bytes) = std::fs::read(&path) {
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".to_owned(), FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("cjk".to_owned());
            }
            ctx.set_fonts(fonts);
        } else {
            warn!("egui 中文字体加载失败: {path}");
        }
        *fonts_loaded = true;
    }

    // 根 viewport Ui（egui 0.36 面板在此内显示）
    let mut viewport_ui = Ui::new(
        ctx.clone(),
        "viewport".into(),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    egui::Panel::left("block_panel")
        .default_size(200.0)
        .resizable(true)
        .show(&mut viewport_ui, |ui| {
            ui.heading("积木库");
            ui.separator();

            // 蓝图模式：完成度进度条
            if blueprint.active {
                ui.label(format!("蓝图：{}", blueprint.def.name));
                let completion = blueprint.completion.clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(completion)
                        .text(format!("完成度 {:.0}%", completion * 100.0)),
                );
                ui.label("仅可放置蓝图期望格（红=不匹配）");
                ui.separator();
            }

            // 积木列表
            let mut clicked: Option<usize> = None;
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, def) in library.defs.iter().enumerate() {
                        let selected = i == library.current;
                        let color = Color32::from_rgba_unmultiplied(
                            (def.color[0] * 255.0) as u8,
                            (def.color[1] * 255.0) as u8,
                            (def.color[2] * 255.0) as u8,
                            255,
                        );
                        let (w, h, d) = (def.size[0], def.size[1], def.size[2]);
                        let mut text = RichText::new(format!("{}  {}×{}×{}", def.name, w, h, d))
                            .color(color);
                        if selected {
                            text = text.strong();
                        }
                        if ui.selectable_label(selected, text).clicked() {
                            clicked = Some(i);
                        }
                    }
                });
            // 循环外应用选择（避免借用冲突）
            if let Some(i) = clicked {
                library.current = i;
            }
        });
}

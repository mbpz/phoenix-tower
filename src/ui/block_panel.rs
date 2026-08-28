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
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};

use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::{select_blueprint, Blueprint, BlueprintGhost, BlueprintLibrary};
use crate::building::challenge::{start_challenge, Challenge, ChallengeState};
use crate::building::placement::{PlacedBlock, PlacedBlocks};

/// 面板 Tab。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PanelTab {
    #[default]
    Blocks,
    Codex,
    Achievements,
}

pub struct BlockPanelPlugin;

impl Plugin for BlockPanelPlugin {
    fn build(&self, app: &mut App) {
        // 必须在 bevy_egui 帧调度（EguiPrimaryContextPass）内绘制：
        // Update 阶段的 Context 尚未 run()，绘制会 panic（"No fonts available..."）
        app.add_systems(EguiPrimaryContextPass, block_panel_ui);
    }
}

fn block_panel_ui(
    mut contexts: EguiContexts,
    mut fonts_loaded: Local<bool>,
    mut tab: Local<PanelTab>,
    mut library: ResMut<BlockLibrary>,
    mut blueprint: ResMut<Blueprint>,
    mut blueprint_library: ResMut<BlueprintLibrary>,
    mut challenge: ResMut<Challenge>,
    collection: Res<crate::building::collection::Collection>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    placed_query: Query<Entity, With<PlacedBlock>>,
    ghost_query: Query<Entity, With<BlueprintGhost>>,
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

    let mut start_requested = false;
    let mut theme_switch: Option<usize> = None;
    egui::Panel::left("block_panel")
        .default_size(210.0)
        .resizable(true)
        .show(&mut viewport_ui, |ui| {
            ui.heading("黄鹤楼积木");
            ui.separator();

            // Tab 切换
            ui.horizontal(|ui| {
                ui.selectable_value(&mut *tab, PanelTab::Blocks, "积木");
                ui.selectable_value(&mut *tab, PanelTab::Codex, "图鉴");
                ui.selectable_value(&mut *tab, PanelTab::Achievements, "成就");
            });
            ui.separator();

            // 挑战模式（B-16）
            ui.label(format!("🏆 挑战：{}", challenge.def.name));
            match challenge.state {
                ChallengeState::Active => {
                    ui.label(format!("⏱ 剩余 {:.0} 秒", challenge.time_left));
                    let (used, total) = challenge.quota_used_total();
                    ui.label(format!("🧱 材料 {used}/{total}"));
                    if ui.button("重新开始（C）").clicked() {
                        start_requested = true;
                    }
                }
                ChallengeState::Won => {
                    ui.label(format!(
                        "✨ 完成：{} ★",
                        "★".repeat(challenge.stars as usize)
                    ));
                    if ui.button("再次挑战（C）").clicked() {
                        start_requested = true;
                    }
                }
                ChallengeState::Failed => {
                    ui.label("⏱ 挑战失败：时间耗尽");
                    if ui.button("重试（C）").clicked() {
                        start_requested = true;
                    }
                }
                ChallengeState::Idle => {
                    ui.label(&challenge.def.description);
                    if ui.button("开始挑战（C）").clicked() {
                        start_requested = true;
                    }
                }
            }
            ui.separator();

            // 蓝图模式：主题选择 + 完成度进度条
            if blueprint.active {
                let mut picked = blueprint_library.current;
                egui::ComboBox::from_label("蓝图主题")
                    .selected_text(blueprint_library.current_def().name.clone())
                    .show_ui(ui, |ui| {
                        for (i, def) in blueprint_library.defs.iter().enumerate() {
                            ui.selectable_value(&mut picked, i, def.name.clone());
                        }
                    });
                if picked != blueprint_library.current {
                    theme_switch = Some(picked);
                }
                ui.label(format!("蓝图：{}", blueprint.def.name));
                let completion = blueprint.completion.clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(completion)
                        .text(format!("完成度 {:.0}%", completion * 100.0)),
                );
                ui.label("仅可放置蓝图期望格（红=不匹配）");
                ui.separator();
            }

            match *tab {
                PanelTab::Blocks => {
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
                                let mut text =
                                    RichText::new(format!("{}  {}×{}×{}", def.name, w, h, d))
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
                }
                PanelTab::Codex => {
                    // 图鉴（B-18）
                    ui.label(format!(
                        "📖 部件图鉴 {}/{}",
                        collection.codex.len(),
                        library.defs.len()
                    ));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for def in &library.defs {
                                if collection.codex.contains(&def.id) {
                                    let (w, h, d) = (def.size[0], def.size[1], def.size[2]);
                                    let color = Color32::from_rgba_unmultiplied(
                                        (def.color[0] * 255.0) as u8,
                                        (def.color[1] * 255.0) as u8,
                                        (def.color[2] * 255.0) as u8,
                                        255,
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "{}（{}） {}×{}×{}",
                                            def.name, def.layer, w, h, d
                                        ))
                                        .color(color)
                                        .strong(),
                                    );
                                    ui.label(&def.description);
                                    if collection.rare.contains(&def.id) {
                                        ui.label(
                                            RichText::new("✨ 稀有：鎏金版已解锁").color(Color32::GOLD),
                                        );
                                    }
                                } else {
                                    ui.label(format!("？？？ — 蓝图复原中放置「{}」解锁", def.name));
                                }
                                ui.separator();
                            }
                        });
                }
                PanelTab::Achievements => {
                    // 成就（B-18）
                    ui.label(format!(
                        "🏅 成就 {}/5",
                        collection.achievements.len()
                    ));
                    ui.separator();
                    for (id, name, desc) in crate::building::collection::achievement_defs() {
                        let unlocked = collection.achievements.contains(id);
                        let mut name_text = RichText::new(name);
                        if unlocked {
                            name_text = name_text.strong();
                        }
                        ui.horizontal(|ui| {
                            ui.label(if unlocked { "✅" } else { "🔒" });
                            ui.label(name_text);
                        });
                        ui.label(desc);
                        ui.separator();
                    }
                }
            }
        });

    // 面板关闭后应用挑战启动 / 主题切换（需要可变借用）
    if let Some(idx) = theme_switch {
        select_blueprint(&mut blueprint, &mut blueprint_library, idx);
        // 切换后销毁旧幽灵蓝图，由对账系统按新蓝图重建
        for e in ghost_query.iter() {
            commands.entity(e).despawn();
        }
        info!("🏯 主题切换：{}", blueprint_library.current_def().name);
    }
    if start_requested {
        start_challenge(
            &mut commands,
            &mut stack,
            &mut blueprint,
            &mut blueprint_library,
            &mut challenge,
            &placed_query,
        );
    }
}

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
use crate::building::challenge::{
    select_challenge, start_challenge, Challenge, ChallengeLibrary, ChallengeState,
};
use crate::building::collection::KnowledgeHints;
use crate::building::placement::{BlockRenderAssets, BlueprintAlpha, PlacedBlock, PlacedBlocks};
use crate::i18n::{t, Locale};
use crate::save::{import_save, latest_ptw, load_save_from_path, SAVE_VERSION};

/// 面板资源打包（Bevy 系统参数上限 16，合并为一组）。
#[derive(bevy::ecs::system::SystemParam)]
struct PanelCtx<'w> {
    library: ResMut<'w, BlockLibrary>,
    blueprint: ResMut<'w, Blueprint>,
    blueprint_library: ResMut<'w, BlueprintLibrary>,
    challenge: ResMut<'w, Challenge>,
    challenge_library: ResMut<'w, ChallengeLibrary>,
    collection: Res<'w, crate::building::collection::Collection>,
    render: Res<'w, BlockRenderAssets>,
    locale: Res<'w, Locale>,
    hints: Res<'w, KnowledgeHints>,
    ghost_alpha: ResMut<'w, BlueprintAlpha>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    stack: ResMut<'w, PlacedBlocks>,
}

/// 面板 Tab。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PanelTab {
    #[default]
    Blocks,
    Codex,
    Achievements,
    Archive,
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
    mut import_path: Local<String>,
    mut ctx: PanelCtx,
    mut commands: Commands,
    placed_query: Query<Entity, With<PlacedBlock>>,
    ghost_query: Query<Entity, With<BlueprintGhost>>,
) {
    let library = &mut ctx.library;
    let mut blueprint = &mut ctx.blueprint;
    let mut blueprint_library = &mut ctx.blueprint_library;
    let mut challenge = &mut ctx.challenge;
    let mut challenge_library = &mut ctx.challenge_library;
    let collection = &ctx.collection;
    let render = &ctx.render;
    let locale = &ctx.locale;
    let hints = &ctx.hints;
    let ghost_alpha = &mut ctx.ghost_alpha;
    let materials = &mut ctx.materials;
    let mut stack = &mut ctx.stack;
    let mut start_requested = false;
    let mut theme_switch: Option<usize> = None;
    let mut save_requested = false;
    let mut share_requested = false;
    let mut json_requested = false;
    let mut import_requested: Option<std::path::PathBuf> = None;
    let challenge_switch: Option<usize> = None;
    let mut alpha_dirty: Option<f32> = None;

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let lang = locale.lang;

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
        .default_size(210.0)
        .resizable(true)
        .show(&mut viewport_ui, |ui| {
            ui.heading(t("黄鹤楼积木", "Tower Blocks", lang));
            ui.separator();

            // Tab 切换
            ui.horizontal(|ui| {
                ui.selectable_value(&mut *tab, PanelTab::Blocks, t("积木", "Blocks", lang));
                ui.selectable_value(&mut *tab, PanelTab::Codex, t("图鉴", "Codex", lang));
                ui.selectable_value(
                    &mut *tab,
                    PanelTab::Achievements,
                    t("成就", "Achievements", lang),
                );
                ui.selectable_value(&mut *tab, PanelTab::Archive, t("存档", "Saves", lang));
            });
            ui.separator();

            // 挑战模式（B-16）
            ui.label(format!(
                "🏆 {}：{}",
                t("挑战", "Challenge", lang),
                challenge.def.name
            ));
            match challenge.state {
                ChallengeState::Active => {
                    ui.label(format!(
                        "⏱ {} {:.0}s",
                        t("剩余", "left", lang),
                        challenge.time_left
                    ));
                    let (used, total) = challenge.quota_used_total();
                    ui.label(format!(
                        "🧱 {} {used}/{total}",
                        t("材料", "materials", lang)
                    ));
                    if ui
                        .button(format!("{}（C）", t("重新开始", "Restart", lang)))
                        .clicked()
                    {
                        start_requested = true;
                    }
                }
                ChallengeState::Won => {
                    ui.label(format!(
                        "✨ {}: {} ★",
                        t("完成", "Won", lang),
                        "★".repeat(challenge.stars as usize)
                    ));
                    if ui
                        .button(format!("{}（C）", t("再次挑战", "Play again", lang)))
                        .clicked()
                    {
                        start_requested = true;
                    }
                }
                ChallengeState::Failed => {
                    ui.label(format!(
                        "⏱ {}: {}",
                        t("挑战失败", "Failed", lang),
                        t("时间耗尽", "time up", lang)
                    ));
                    if ui
                        .button(format!("{}（C）", t("重试", "Retry", lang)))
                        .clicked()
                    {
                        start_requested = true;
                    }
                }
                ChallengeState::Idle => {
                    ui.label(&challenge.def.description);
                    if ui
                        .button(format!("{}（C）", t("开始挑战", "Start Challenge", lang)))
                        .clicked()
                    {
                        start_requested = true;
                    }
                }
            }
            ui.separator();

            // 蓝图模式：主题选择 + 完成度进度条
            if blueprint.active {
                let mut picked = blueprint_library.current;
                egui::ComboBox::from_label(t("蓝图主题", "Blueprint Theme", lang))
                    .selected_text(blueprint_library.current_def().name.clone())
                    .show_ui(ui, |ui| {
                        for (i, def) in blueprint_library.defs.iter().enumerate() {
                            ui.selectable_value(&mut picked, i, def.name.clone());
                        }
                    });
                if picked != blueprint_library.current {
                    theme_switch = Some(picked);
                }
                ui.label(format!(
                    "{}: {}",
                    t("蓝图", "Blueprint", lang),
                    blueprint.def.name
                ));
                let completion = blueprint.completion.clamp(0.0, 1.0);
                ui.add(egui::ProgressBar::new(completion).text(format!(
                    "{} {:.0}%",
                    t("完成度", "Done", lang),
                    completion * 100.0
                )));
                ui.label(t(
                    "仅可放置蓝图期望格（红=不匹配）",
                    "Only blueprint cells (red = mismatch)",
                    lang,
                ));
                // 蓝图透明度滑杆（B-10 打磨）：实时调整幽灵蓝图透明度
                let mut alpha = ghost_alpha.value;
                if ui
                    .add(egui::Slider::new(&mut alpha, 0.1..=0.8).text(t(
                        "蓝图透明度",
                        "Ghost opacity",
                        lang,
                    )))
                    .changed()
                {
                    alpha_dirty = Some(alpha);
                }
                ui.separator();
            }

            // 知识卡片（PRD §3.3 智能提示）
            if let Some(card) = &hints.card {
                ui.group(|ui| {
                    ui.label(RichText::new(format!("📖 {}", card.name)).strong());
                    ui.label(&card.desc);
                });
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
                        "📖 {} {}/{}",
                        t("部件图鉴", "Codex", lang),
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
                                            RichText::new("✨ 稀有：鎏金版已解锁")
                                                .color(Color32::GOLD),
                                        );
                                    }
                                } else {
                                    ui.label(format!(
                                        "？？？ — {}「{}」",
                                        t(
                                            "蓝图复原中放置此部件解锁",
                                            "Place this part in blueprint mode to unlock",
                                            lang
                                        ),
                                        def.name
                                    ));
                                }
                                ui.separator();
                            }
                        });
                }
                PanelTab::Achievements => {
                    // 成就（B-18）
                    ui.label(format!(
                        "🏅 {} {}/5",
                        t("成就", "Achievements", lang),
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
                PanelTab::Archive => {
                    // 存档与分享（B-23）
                    ui.label(format!(
                        "{} v{SAVE_VERSION} .ptw",
                        t("格式版本", "Format", lang)
                    ));
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .button(format!("💾 {} (F5)", t("保存", "Save", lang)))
                            .clicked()
                        {
                            save_requested = true;
                        }
                        if ui
                            .button(format!("📤 {} (F7)", t("分享导出", "Share", lang)))
                            .clicked()
                        {
                            share_requested = true;
                        }
                        if ui
                            .button(format!("📄 JSON {} (F6)", t("导出", "Export", lang)))
                            .clicked()
                        {
                            json_requested = true;
                        }
                    });
                    ui.separator();
                    ui.label(t(
                        "存档列表（点击加载）",
                        "Save files (click to load):",
                        lang,
                    ));
                    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("saves");
                    let files = latest_ptw(&dir)
                        .map(|_| {
                            std::fs::read_dir(&dir)
                                .map(|rd| {
                                    let mut v: Vec<_> = rd
                                        .filter_map(Result::ok)
                                        .map(|e| e.path())
                                        .filter(|p| p.extension().is_some_and(|e| e == "ptw"))
                                        .collect();
                                    v.sort();
                                    v
                                })
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                    for f in &files {
                        let name = f
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if ui.button(format!("📂 {name}")).clicked() {
                            import_requested = Some(f.clone());
                        }
                    }
                    if files.is_empty() {
                        ui.label(t(
                            "（暂无存档，先按 F5 保存）",
                            "(No saves yet - press F5)",
                            lang,
                        ));
                    }
                    ui.separator();
                    ui.label(t("自定义路径导入", "Import from path", lang));
                    ui.text_edit_singleline(&mut *import_path);
                    if ui
                        .button(format!("{} .ptw", t("导入", "Import", lang)))
                        .clicked()
                    {
                        let p = import_path.trim().to_string();
                        if !p.is_empty() {
                            import_requested = Some(std::path::PathBuf::from(p));
                        }
                    }
                }
            }
        });

    // 面板关闭后应用动作（需要可变借用）
    if let Some(a) = alpha_dirty {
        for handle in render.blueprint_materials.values() {
            if let Some(mut mat) = materials.get_mut(handle) {
                mat.base_color.set_alpha(a);
            }
        }
        ghost_alpha.value = a;
    }
    if let Some(idx) = theme_switch {
        select_blueprint(&mut blueprint, &mut blueprint_library, idx);
        // 切换后销毁旧幽灵蓝图，由对账系统按新蓝图重建
        for e in ghost_query.iter() {
            commands.entity(e).despawn();
        }
        info!("🏯 主题切换：{}", blueprint_library.current_def().name);
    }
    if let Some(path) = import_requested {
        match load_save_from_path(&path) {
            Ok(save) => {
                let n = import_save(
                    &mut commands,
                    &mut stack,
                    &library,
                    &render,
                    &mut blueprint,
                    &mut challenge,
                    &placed_query,
                    save,
                );
                info!("📂 已导入 {}（{n} 个积木）", path.display());
            }
            Err(e) => error!("导入失败: {e}"),
        }
    }
    if save_requested || share_requested || json_requested {
        let mode = if blueprint.active {
            "blueprint"
        } else {
            "free"
        };
        let save = crate::save::build_save(&stack, &blueprint, mode);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("saves");
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if save_requested {
            if let (Ok(bytes), true) = (
                crate::save::encode_bincode(&save),
                std::fs::create_dir_all(&dir).is_ok(),
            ) {
                match crate::save::write_atomic(&dir.join("slot1.ptw"), &bytes) {
                    Ok(()) => info!("💾 已保存（{} 个积木）", save.meta.block_count),
                    Err(e) => error!("保存失败: {e}"),
                }
            }
        }
        if share_requested {
            let path = dir.join(format!("share_{unix}.ptw"));
            if let Ok(bytes) = crate::save::encode_bincode(&save) {
                match crate::save::write_atomic(&path, &bytes) {
                    Ok(()) => info!("📤 分享存档已导出：{}", path.display()),
                    Err(e) => error!("分享导出失败: {e}"),
                }
            }
        }
        if json_requested {
            if let Ok(json) = serde_json::to_string_pretty(&save) {
                match std::fs::write(dir.join("export.json"), json) {
                    Ok(()) => info!("📄 已导出 saves/export.json"),
                    Err(e) => error!("JSON 导出失败: {e}"),
                }
            }
        }
    }
    if let Some(idx) = challenge_switch {
        select_challenge(&mut challenge, &mut challenge_library, idx);
        info!("🏆 挑战切换：{}", challenge_library.current_def().name);
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

//! Save-tab layout, path input, and observers forwarding to the existing save API.
use super::{LunexTab, LunexTabId, LunexTheme, TabSavesRoot};
use crate::building::block_defs::BlockLibrary;
use crate::building::blueprint::Blueprint;
use crate::building::challenge::Challenge;
use crate::building::placement::{BlockRenderAssets, PlacedBlock, PlacedBlocks};
use crate::save::{
    export_json, import_latest, import_save, load_save_from_path, load_slot, save_file_list,
    save_slot, saves_dir, share_export,
};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy_lunex::prelude::*;
use bevy_lunex::UiSelected;
use std::path::Path;

// ---- 存档 Tab ----

/// 存档文件列表文本（多行）
#[derive(Component)]
pub struct SaveListText;

/// 路径输入框（可点击聚焦）
#[derive(Component)]
pub struct PathInputBox;

/// 路径输入框文本
#[derive(Component)]
pub struct PathInputText;

/// 自定义路径导入输入状态
#[derive(Resource, Default)]
pub struct PathInput {
    pub value: String,
    pub focused: bool,
}

/// 存档列表：每 1 秒刷新（仅存档 Tab 激活时）
pub(super) fn save_list_system(
    time: Res<Time>,
    tab: Res<LunexTab>,
    mut last: Local<f32>,
    mut texts: Query<&mut Text2d, With<SaveListText>>,
) {
    if tab.0 != LunexTabId::Saves {
        return;
    }
    if time.elapsed_secs() - *last < 1.0 {
        return;
    }
    *last = time.elapsed_secs();
    let entries = save_file_list(&saves_dir());
    let txt = if entries.is_empty() {
        "（暂无存档）".to_string()
    } else {
        entries
            .iter()
            .map(|(n, b, t)| format!("{n} · {b} 块 · {t}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    for t in &mut texts {
        t.map_unchanged(|t| &mut t.0).set_if_neq(txt.clone());
    }
}

/// 路径输入：聚焦时接收键盘（字符/退格/Esc/Enter）
pub(super) fn path_input_system(
    mut input: ResMut<PathInput>,
    mut keys: MessageReader<KeyboardInput>,
    mut texts: Query<&mut Text2d, With<PathInputText>>,
    mut boxes: Query<&mut UiSelected, With<PathInputBox>>,
) {
    if input.focused {
        for msg in keys.read() {
            if msg.state != ButtonState::Pressed {
                continue;
            }
            match &msg.logical_key {
                Key::Character(c) => input.value.push_str(c),
                Key::Backspace => {
                    input.value.pop();
                }
                Key::Escape => input.focused = false,
                Key::Enter => input.focused = false,
                _ => {}
            }
        }
    }
    for t in &mut texts {
        let value = if input.value.is_empty() {
            "输入路径…".to_string()
        } else {
            input.value.clone()
        };
        t.map_unchanged(|t| &mut t.0).set_if_neq(value);
    }
    for s in &mut boxes {
        s.map_unchanged(|s| &mut s.0)
            .set_if_neq(if input.focused { 1.0 } else { 0.0 });
    }
}

/// 路径输入框点击 → 聚焦
fn path_box_click(trigger: On<Pointer<Click>>, mut input: ResMut<PathInput>) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    input.focused = true;
}

/// 导入自定义路径存档
fn path_import_click(
    trigger: On<Pointer<Click>>,
    input: Res<PathInput>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed: Query<Entity, With<PlacedBlock>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let path_str = input.value.trim();
    if path_str.is_empty() {
        info!("💾 请先输入存档路径");
        return;
    }
    match load_save_from_path(Path::new(path_str)) {
        Ok(save) => {
            let loaded = import_save(
                &mut commands,
                &mut stack,
                &library,
                &render,
                &mut blueprint,
                &mut challenge,
                &placed,
                save,
            );
            info!("📂 已导入 {path_str}（{loaded} 个积木）");
        }
        Err(e) => error!("导入失败: {e}"),
    }
}

/// 保存按钮（F5 同流程）
fn save_button_click(
    trigger: On<Pointer<Click>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<Blueprint>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = save_slot(&stack, &blueprint) {
        error!("保存失败: {e}");
    }
}

/// JSON 导出按钮（F6 同流程）
fn json_button_click(
    trigger: On<Pointer<Click>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<Blueprint>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = export_json(&stack, &blueprint) {
        error!("JSON 导出失败: {e}");
    }
}

/// 分享按钮（F7 同流程）
fn share_button_click(
    trigger: On<Pointer<Click>>,
    stack: Res<PlacedBlocks>,
    blueprint: Res<Blueprint>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = share_export(&stack, &blueprint) {
        error!("分享导出失败: {e}");
    }
}

/// 加载槽位按钮（F9 同流程）
fn load_slot_button_click(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed: Query<Entity, With<PlacedBlock>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = load_slot(
        &mut commands,
        &mut stack,
        &library,
        &render,
        &mut blueprint,
        &mut challenge,
        &placed,
    ) {
        info!("💾 {e}");
    }
}

/// 导入最新分享按钮（F8 同流程）
fn import_latest_button_click(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut challenge: ResMut<Challenge>,
    placed: Query<Entity, With<PlacedBlock>>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    if let Err(e) = import_latest(
        &mut commands,
        &mut stack,
        &library,
        &render,
        &mut blueprint,
        &mut challenge,
        &placed,
    ) {
        info!("📂 {e}");
    }
}

pub(super) fn spawn_saves_tab(
    panel: &mut ChildSpawnerCommands,
    font: &FontSource,

    materials: &mut Assets<ColorMaterial>,
    theme: &LunexTheme,
) {
    // 存档 Tab 内容（默认隐藏；Tab 切换显示）
    panel
        .spawn((
            Name::new("Saves Tab"),
            UiLayout::window()
                .pos((Rl(50.0), Rh(10.5)))
                .size((Rl(96.0), Rh(84.0)))
                .anchor(Anchor::TOP_CENTER)
                .pack(),
            Pickable::IGNORE,
            Visibility::Hidden,
            TabSavesRoot,
        ))
        .with_children(|sv| {
            // 行 1：保存 / JSON / 分享（观察者签名各异，逐个生成）
            let save_mat = materials.add(ColorMaterial::from(theme.row_base));
            sv.spawn((
                Name::new("save_btn"),
                UiLayout::window()
                    .pos((Rl(16.5), Rh(10.0)))
                    .size((Rl(30.0), Rh(8.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiHover::id(), theme.row_hover),
                ]),
                UiHover::new().instant(true),
                UiMeshPlane2d,
                MeshMaterial2d(save_mat),
                Pickable::default(),
            ))
            .observe(hover_set::<Pointer<Over>, true>)
            .observe(hover_set::<Pointer<Out>, false>)
            .observe(save_button_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("btn_text"),
                    Text2d::new("保存"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(48.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window().full().pack(),
                    Pickable::IGNORE,
                ));
            });
            let json_mat = materials.add(ColorMaterial::from(theme.row_base));
            sv.spawn((
                Name::new("json_btn"),
                UiLayout::window()
                    .pos((Rl(50.0), Rh(10.0)))
                    .size((Rl(30.0), Rh(8.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiHover::id(), theme.row_hover),
                ]),
                UiHover::new().instant(true),
                UiMeshPlane2d,
                MeshMaterial2d(json_mat),
                Pickable::default(),
            ))
            .observe(hover_set::<Pointer<Over>, true>)
            .observe(hover_set::<Pointer<Out>, false>)
            .observe(json_button_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("btn_text"),
                    Text2d::new("JSON"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(48.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window().full().pack(),
                    Pickable::IGNORE,
                ));
            });
            let share_mat = materials.add(ColorMaterial::from(theme.row_base));
            sv.spawn((
                Name::new("share_btn"),
                UiLayout::window()
                    .pos((Rl(83.5), Rh(10.0)))
                    .size((Rl(30.0), Rh(8.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiHover::id(), theme.row_hover),
                ]),
                UiHover::new().instant(true),
                UiMeshPlane2d,
                MeshMaterial2d(share_mat),
                Pickable::default(),
            ))
            .observe(hover_set::<Pointer<Over>, true>)
            .observe(hover_set::<Pointer<Out>, false>)
            .observe(share_button_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("btn_text"),
                    Text2d::new("分享"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(48.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window().full().pack(),
                    Pickable::IGNORE,
                ));
            });
            // 行 2：加载槽位 / 导入最新
            let load_mat = materials.add(ColorMaterial::from(theme.row_base));
            sv.spawn((
                Name::new("load_slot_btn"),
                UiLayout::window()
                    .pos((Rl(25.0), Rh(24.0)))
                    .size((Rl(46.0), Rh(8.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiHover::id(), theme.row_hover),
                ]),
                UiHover::new().instant(true),
                UiMeshPlane2d,
                MeshMaterial2d(load_mat),
                Pickable::default(),
            ))
            .observe(hover_set::<Pointer<Over>, true>)
            .observe(hover_set::<Pointer<Out>, false>)
            .observe(load_slot_button_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("btn_text"),
                    Text2d::new("加载槽位"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(48.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window().full().pack(),
                    Pickable::IGNORE,
                ));
            });
            let il_mat = materials.add(ColorMaterial::from(theme.row_base));
            sv.spawn((
                Name::new("import_latest_btn"),
                UiLayout::window()
                    .pos((Rl(75.0), Rh(24.0)))
                    .size((Rl(46.0), Rh(8.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiHover::id(), theme.row_hover),
                ]),
                UiHover::new().instant(true),
                UiMeshPlane2d,
                MeshMaterial2d(il_mat),
                Pickable::default(),
            ))
            .observe(hover_set::<Pointer<Over>, true>)
            .observe(hover_set::<Pointer<Out>, false>)
            .observe(import_latest_button_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("btn_text"),
                    Text2d::new("导入最新"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(48.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window().full().pack(),
                    Pickable::IGNORE,
                ));
            });
            // 文件列表标签 + 多行列表
            sv.spawn((
                Name::new("Save List Label"),
                Text2d::new("存档文件"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                UiTextSize::from(Rh(2.8)),
                UiColor::new(vec![(UiBase::id(), theme.accent)]),
                UiLayout::window()
                    .pos((Rl(4.0), Rh(36.0)))
                    .anchor(Anchor::TOP_LEFT)
                    .pack(),
                Pickable::IGNORE,
            ));
            sv.spawn((
                Name::new("Save List Text"),
                Text2d::new("（暂无存档）"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                UiTextSize::from(Rh(2.6)),
                UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                UiLayout::window()
                    .pos((Rl(4.0), Rh(41.0)))
                    .anchor(Anchor::TOP_LEFT)
                    .pack(),
                Pickable::IGNORE,
                SaveListText,
            ));
            // 路径导入行：输入框 + 导入按钮
            let box_mat = materials.add(ColorMaterial::from(theme.row_base));
            sv.spawn((
                Name::new("Path Input Box"),
                UiLayout::window()
                    .pos((Rl(30.0), Rh(88.0)))
                    .size((Rl(58.0), Rh(7.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiSelected::id(), theme.row_selected),
                ]),
                UiSelected(0.0),
                UiMeshPlane2d,
                MeshMaterial2d(box_mat),
                Pickable::default(),
                PathInputBox,
            ))
            .observe(path_box_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("Path Input Text"),
                    Text2d::new("输入路径…"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(46.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window()
                        .pos((Rl(6.0), Rl(50.0)))
                        .anchor(Anchor::CENTER_LEFT)
                        .pack(),
                    Pickable::IGNORE,
                    PathInputText,
                ));
            });
            let imp_mat = materials.add(ColorMaterial::from(theme.row_base));
            sv.spawn((
                Name::new("Path Import Button"),
                UiLayout::window()
                    .pos((Rl(72.0), Rh(88.0)))
                    .size((Rl(30.0), Rh(7.0)))
                    .anchor(Anchor::CENTER)
                    .pack(),
                UiColor::new(vec![
                    (UiBase::id(), theme.row_base),
                    (UiHover::id(), theme.row_hover),
                ]),
                UiHover::new().instant(true),
                UiMeshPlane2d,
                MeshMaterial2d(imp_mat),
                Pickable::default(),
            ))
            .observe(hover_set::<Pointer<Over>, true>)
            .observe(hover_set::<Pointer<Out>, false>)
            .observe(path_import_click)
            .with_children(|b| {
                b.spawn((
                    Name::new("path_import_text"),
                    Text2d::new("导入"),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(48.0)),
                    UiColor::new(vec![(UiBase::id(), theme.text_main)]),
                    UiLayout::window().full().pack(),
                    Pickable::IGNORE,
                ));
            });
        });
}

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

use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy_lunex::prelude::*;
use bevy_rich_text3d::{LoadFonts, Text3d, Text3dStyling};

pub struct LunexUiPlugin;

impl Plugin for LunexUiPlugin {
    fn build(&self, app: &mut App) {
        // 让 bevy_rich_text3d 的字库包含我们的 CJK 子集字体（A2 验证 + C1 匾额用）。
        // Text3dPlugin 在插件组 cleanup 时读取本资源；init_resource 不覆盖已存在的值，
        // 因此这里先填充再 add_plugins。
        app.init_resource::<LoadFonts>();
        let subset = format!("{}/assets/fonts/NotoSansSC-subset.otf", env!("CARGO_MANIFEST_DIR"));
        app.world_mut()
            .resource_mut::<LoadFonts>()
            .font_paths
            .push(subset);
        app.add_plugins(UiLunexPlugins).add_systems(
            Startup,
            (spawn_ui_camera, spawn_hud_root, spawn_text3d_probe),
        );
        // 启动后输出一次 UI 管线诊断（A1/A2 冒烟验证；PHOENIX_UI_PROBE=1 开启）。
        // 无图形权限环境无法截图 2D 层，用组件状态证明布局/文本/网格已产出。
        // 2026-09 验证结果：root dimension 由相机视口注入（1280×720）；
        // UiMeshPlane2d 横幅 mesh+material 产出；Text2d 中文排布 372×53；
        // Text3d 中文网格产出 dim=7.59×0.9（CJK 子集字体经 LoadFonts 注入）。
        app.add_systems(Update, ui_probe_diagnostic);
    }
}

/// 冒烟诊断：打印 lunex 布局、2D 文本排布、3D 文本网格的实际状态。
fn ui_probe_diagnostic(
    time: Res<Time>,
    mut done: Local<bool>,
    roots: Query<&Dimension, (With<UiLayoutRoot>, With<UiFetchFromCamera<0>>)>,
    banners: Query<
        (Entity, Option<&Mesh2d>, Option<&MeshMaterial2d<ColorMaterial>>),
        With<UiMeshPlane2d>,
    >,
    texts: Query<(&Text2d, &TextLayoutInfo), With<UiTextSize>>,
    probes: Query<
        (
            Entity,
            &Transform,
            Option<&Mesh3d>,
            Option<&bevy_rich_text3d::Text3dDimensionOut>,
        ),
        With<Text3dProbe>,
    >,
    renderer: Option<Res<bevy_rich_text3d::TextRenderer>>,
) {
    if std::env::var("PHOENIX_UI_PROBE").is_err() || *done || time.elapsed_secs() < 4.0 {
        return;
    }
    *done = true;
    info!(
        "🧪 rich_text3d TextRenderer resource exists: {}",
        renderer.is_some()
    );
    for d in &roots {
        info!("🧪 lunex root dimension = {d:?}");
    }
    for (e, mesh, mat) in &banners {
        info!(
            "🧪 lunex banner {e:?}: mesh2d={} material2d={}",
            mesh.is_some(),
            mat.is_some()
        );
    }
    for (t, layout) in &texts {
        info!("🧪 lunex text \"{}\" layout={:?}", t.0, layout.size);
    }
    for (e, tf, mesh, dim) in &probes {
        info!(
            "🧪 text3d probe {e:?} at {:?}: mesh3d={} dim={:?}",
            tf.translation,
            mesh.is_some(),
            dim.map(|d| d.dimension)
        );
    }
}

/// 2D UI 相机：叠加在 3D 主相机之上（order 更高）、透明清屏，
/// 作为 lunex 布局的尺寸来源（`UiSourceCamera::<0>`）。
///
/// 说明：bevy_ui 的 HUD 提示（hud.rs）会改由这台相机绘制
/// （bevy_ui 选择「指向主窗口的 order 最高相机」），行为不变。
fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Lunex UiCamera"),
        Camera2d,
        Camera {
            // 不清屏：让 3D 画面透过来，本相机只画 2D UI 层
            clear_color: ClearColorConfig::None,
            // 高于 3D 主相机（默认 order 0），渲染在场景之上
            order: 1,
            ..default()
        },
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
    ));
}

/// 最小界面：顶部标题横幅（验证布局 + CJK 文本 + 渲染管线）。
/// 后续 B 阶段逐 Tab 迁移时，egui 面板仍并行保留（feature 切换）。
fn spawn_hud_root(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let font = FontSource::Handle(asset_server.load("fonts/NotoSansSC-subset.otf"));
    // lunex 只重建 Mesh2d 几何，材质需自行提供（UiColor 系统负责着色）
    let banner_material = materials.add(ColorMaterial::from(Color::BLACK));

    commands
        .spawn((
            Name::new("Lunex HUD Root"),
            UiLayoutRoot::new_2d(),
            UiFetchFromCamera::<0>,
        ))
        .with_children(|ui| {
            ui.spawn((
                Name::new("Title Banner"),
                // 顶部居中横幅：pos 为锚点位置，size 为相对父节点（视口）比例
                UiLayout::window()
                    .pos((Rl(50.0), Rh(6.0)))
                    .size((Rl(36.0), Rh(6.5)))
                    .anchor(Anchor::TOP_CENTER)
                    .pack(),
                UiColor::new(vec![(UiBase::id(), Color::srgba(0.09, 0.13, 0.19, 0.82))]),
                UiMeshPlane2d,
                MeshMaterial2d(banner_material.clone()),
            ))
            .with_children(|banner| {
                banner.spawn((
                    Name::new("Title Text"),
                    Text2d::new("黄鹤楼 · 筑梦江城"),
                    TextFont {
                        font,
                        font_size: FontSize::Px(44.0),
                        ..default()
                    },
                    UiTextSize::from(Rh(55.0)),
                    UiColor::new(vec![(UiBase::id(), Color::srgb(0.95, 0.90, 0.75))]),
                    UiLayout::window().full().pack(),
                    // 纯展示文本，不参与点击
                    Pickable::IGNORE,
                ));
            });
        });
}

/// A2 验证探针（PHOENIX_TEXT3D_PROBE=1 时生成）：世界空间 `Text3d`
/// （bevy_rich_text3d / cosmic-text）渲染中文。默认不生成 —— A2 已验证通过，
/// C1（匾额）正式实现时复用此处的字体注入 + Mesh3d/材质模式。
#[derive(Component)]
struct Text3dProbe;

fn spawn_text3d_probe(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if std::env::var("PHOENIX_TEXT3D_PROBE").is_err() {
        return;
    }
    commands.spawn((
        Name::new("CJK Text3d Probe"),
        Text3d::new("黄鹤楼 · 筑梦江城"),
        Text3dStyling {
            size: 0.9,
            font: "Noto Sans CJK SC".into(),
            color: bevy::color::Srgba::new(0.95, 0.85, 0.40, 1.0),
            align: bevy_rich_text3d::TextAlign::Center,
            ..default()
        },
        // bevy_rich_text3d 需要实体自带 Mesh3d/Mesh2d + 引用字图集的材质
        Mesh3d::default(),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(bevy_rich_text3d::TextAtlas::DEFAULT_IMAGE.clone()),
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        // 塔前空中悬浮（A2 验证后由 C1 正式实现匾额时移除）
        Transform::from_xyz(0.0, 9.0, 7.0),
        Text3dProbe,
    ));
}

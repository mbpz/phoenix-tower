//! Opt-in smoke diagnostics and the CJK Text3d rendering probe.
use super::{
    AchievementNameText, ChallengeButtonText, ChallengeNameText, ChallengeStatusText,
    CodexDetailText, CodexRow, KnowledgeCardName, KnowledgeCardRoot, OpacityFill, PaletteRow,
    ProgressText, TabAchievementsRoot, TabButton, TabCodexRoot, TabSavesRoot, ThemeButtonText,
    ThemeMenuRow,
};
use crate::building::placement::PlaqueText;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy_lunex::prelude::*;
use bevy_lunex::UiSelected;
use bevy_rich_text3d::{Text3d, Text3dStyling};

type BannerMeshQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static Mesh2d>,
        Option<&'static MeshMaterial2d<ColorMaterial>>,
    ),
    With<UiMeshPlane2d>,
>;

type Text3dProbeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static Mesh3d>,
        Option<&'static bevy_rich_text3d::Text3dDimensionOut>,
    ),
    With<Text3dProbe>,
>;

/// 冒烟诊断：打印 lunex 布局、2D 文本排布、3D 文本网格的实际状态。
pub(super) fn ui_probe_diagnostic(
    time: Res<Time>,
    mut done: Local<bool>,
    roots: Query<&Dimension, (With<UiLayoutRoot>, With<UiFetchFromCamera<0>>)>,
    banners: BannerMeshQuery<'_, '_>,
    texts: Query<(&Text2d, &TextLayoutInfo), With<UiTextSize>>,
    probes: Text3dProbeQuery<'_, '_>,
    palette: Query<(&PaletteRow, &UiSelected, &Visibility)>,
    theme_texts: Query<&Text2d, With<ThemeButtonText>>,
    progress_texts: Query<&Text2d, With<ProgressText>>,
    menu_rows: Query<(&ThemeMenuRow, &Visibility)>,
    challenge_status: Query<&Text2d, With<ChallengeStatusText>>,
    challenge_buttons: Query<&Text2d, With<ChallengeButtonText>>,
    challenge_names: Query<&Text2d, With<ChallengeNameText>>,
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
            "🧪 lunex plane {e:?}: mesh2d={} material2d={}",
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
    if !palette.is_empty() {
        let mut rows: Vec<_> = palette.iter().map(|(r, s, v)| (r.0, s.0, *v)).collect();
        rows.sort_by_key(|(i, ..)| *i);
        let visible = rows
            .iter()
            .filter(|(_, _, v)| *v != Visibility::Hidden)
            .count();
        info!(
            "🧪 palette rows: {} total / {} visible; selected = {:?}",
            rows.len(),
            visible,
            rows.iter().find(|(_, s, _)| *s > 0.5).map(|(i, _, _)| *i)
        );
    }
    for t in &theme_texts {
        info!("🧪 theme button: \"{}\"", t.0);
    }
    for t in &progress_texts {
        info!("🧪 progress text: \"{}\"", t.0);
    }
    let open = menu_rows
        .iter()
        .filter(|(_, v)| *v != Visibility::Hidden)
        .count();
    info!("🧪 theme menu rows visible: {open}");
    for t in &challenge_names {
        info!("🧪 challenge name: \"{}\"", t.0);
    }
    for t in &challenge_status {
        info!("🧪 challenge status: \"{}\"", t.0);
    }
    for t in &challenge_buttons {
        info!("🧪 challenge button: \"{}\"", t.0);
    }
}

pub(super) fn ui_probe_tabs(
    time: Res<Time>,
    mut done: Local<bool>,
    tabs: Query<(&TabButton, &UiSelected)>,
    saves_vis: Query<&Visibility, With<TabSavesRoot>>,
    codex_rows: Query<(&CodexRow, &Visibility)>,
    codex_detail: Query<&Text2d, With<CodexDetailText>>,
    codex_vis: Query<&Visibility, With<TabCodexRoot>>,
    ach_vis: Query<&Visibility, With<TabAchievementsRoot>>,
    ach_texts: Query<(&AchievementNameText, &Text2d)>,
    kcard_vis: Query<&Visibility, With<KnowledgeCardRoot>>,
    kcard_names: Query<&Text2d, With<KnowledgeCardName>>,
    op_fills: Query<&UiLayout, With<OpacityFill>>,
    plaques: Query<(Entity, Option<&Mesh3d>), With<PlaqueText>>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    if std::env::var("PHOENIX_UI_PROBE").is_err() || *done || time.elapsed_secs() < 4.0 {
        return;
    }
    *done = true;
    let mut tabs: Vec<_> = tabs
        .iter()
        .map(|(b, s)| (format!("{:?}", b.0), s.0))
        .collect();
    tabs.sort_by(|a, b| a.0.cmp(&b.0));
    info!("🧪 tabs: {:?}", tabs);
    for v in &saves_vis {
        info!("🧪 saves tab visible: {}", *v != Visibility::Hidden);
    }
    if !codex_rows.is_empty() {
        let vis = codex_rows
            .iter()
            .filter(|(_, v)| *v != Visibility::Hidden)
            .count();
        info!(
            "🧪 codex rows: {} total / {vis} visible",
            codex_rows.iter().count()
        );
    }
    for t in &codex_detail {
        info!("🧪 codex detail: \"{}\"", t.0);
    }
    for v in &codex_vis {
        info!("🧪 codex tab visible: {}", *v != Visibility::Hidden);
    }
    for v in &ach_vis {
        info!("🧪 achievements tab visible: {}", *v != Visibility::Hidden);
    }
    let mut ach: Vec<_> = ach_texts.iter().map(|(r, t)| (r.0, t.0.clone())).collect();
    ach.sort_by_key(|(i, _)| *i);
    for (i, t) in ach.iter().take(2) {
        info!(
            "🧪 achievement[{i}]: \"{}\"",
            t.split('\n').next().unwrap_or("")
        );
    }
    for v in &kcard_vis {
        info!("🧪 knowledge card visible: {}", *v != Visibility::Hidden);
    }
    for t in &kcard_names {
        if !t.0.is_empty() {
            info!("🧪 knowledge card: \"{}\"", t.0);
        }
    }
    for f in &op_fills {
        let w = f.layouts.get(&UiBase::id()).and_then(|l| match l {
            bevy_lunex::UiLayoutType::Window(w) => Some(w.size),
            _ => None,
        });
        info!("🧪 opacity fill size: {:?}", w);
    }
    for (e, m) in &plaques {
        info!("🧪 plaque text3d {e:?}: mesh={}", m.is_some());
    }
    let fps = diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    info!("🧪 fps with lunex panel: {fps:.0}");
}

/// A2 验证探针（PHOENIX_TEXT3D_PROBE=1 时生成）：世界空间 `Text3d`
/// （bevy_rich_text3d / cosmic-text）渲染中文。默认不生成 —— A2 已验证通过，
/// C1（匾额）正式实现时复用此处的字体注入 + Mesh3d/材质模式。
#[derive(Component)]
pub(super) struct Text3dProbe;

pub(super) fn spawn_text3d_probe(
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

/// Explicit continuous sampling for ordinary/riverside scenes. Registered only
/// by PHOENIX_PERF_PROBE; stress mode keeps its own sampler (no duplicate samples).
/// Real time, not the clamped virtual clock, sets the one-second cadence.
pub(super) fn ui_perf_probe(
    time: Res<Time<Real>>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut last_log: Local<f64>,
) {
    if time.elapsed_secs_f64() - *last_log < 1.0 {
        return;
    }
    *last_log = time.elapsed_secs_f64();
    if let Some(fps) = diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.value())
        .filter(|fps| fps.is_finite() && *fps > 0.0)
    {
        // Instantaneous frame FPS sampled at 1Hz, not a whole-window average or GPU timing.
        info!("🧪 基准采样：FPS {fps:.1} source=ui");
    }
}

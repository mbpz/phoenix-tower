//! 压力测试模式（B-20）：`PHOENIX_STRESS=<n>` 环境变量启动时自动生成 n 块积木，
//! 每 2 秒输出一次 FPS 采样日志，用于性能基线测量（B-20 验收的量化依据）。

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use crate::building::block_defs::BlockLibrary;
use crate::building::placement::{block_center, rotation_quat, BlockRenderAssets, PlacedBlock};

#[derive(Resource)]
pub struct StressConfig {
    pub count: usize,
}

/// 压力测试积木标记（F9 读档时会一并清空）
#[derive(Component)]
pub struct StressBlock;

pub struct StressPlugin;

impl Plugin for StressPlugin {
    fn build(&self, app: &mut App) {
        let count: usize = std::env::var("PHOENIX_STRESS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if count > 0 {
            app.insert_resource(StressConfig { count })
                .add_systems(Update, (stress_spawn, stress_fps_logger));
        }
    }
}

/// 简单确定性伪随机（避免引入 rand 依赖）。
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

/// 第 3 帧起批量生成随机积木（等待 Startup 资产就绪）。
fn stress_spawn(
    mut frames: Local<u32>,
    mut spawned: Local<bool>,
    config: Res<StressConfig>,
    mut commands: Commands,
    library: Res<BlockLibrary>,
    render: Res<BlockRenderAssets>,
) {
    if *spawned {
        return;
    }
    *frames += 1;
    if *frames < 3 {
        return;
    }
    *spawned = true;

    let mut rng = Lcg(0x9E37_79B9_7F4A_C715);
    info!("🧪 压力测试：生成 {} 块积木", config.count);
    for _ in 0..config.count {
        let x = (rng.next() % 81) as i32 - 40;
        let z = (rng.next() % 81) as i32 - 40;
        let y = (rng.next() % 4) as i32;
        let rot = (rng.next() % 4) as u8;
        let def = &library.defs[(rng.next() as usize) % library.defs.len()];
        let (mesh, mat) = render.per_def.get(&def.id).expect("积木资产应已预生成");
        let anchor = IVec3::new(x, y, z);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(block_center(anchor, def, rot))
                .with_rotation(rotation_quat(rot)),
            PlacedBlock,
            StressBlock,
            Name::new(format!("StressBlock:{}", def.id)),
        ));
    }
}

/// 每 2 秒记录一次当前 FPS。
fn stress_fps_logger(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut last_log: Local<f32>,
) {
    if time.elapsed_secs() - *last_log < 2.0 {
        return;
    }
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    info!("🧪 基准采样：FPS {fps:.1}");
    *last_log = time.elapsed_secs();
}

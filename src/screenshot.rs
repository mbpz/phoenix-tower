//! 截图导出（B-21）：F2 一键截图 PNG 到 saves/。
//!
//! 方案：**离屏渲染目标**——专用 CaptureCamera 渲染到 Image 资产，
//! 用 `Screenshot::image(handle)` 读回保存。
//! （原生 swapchain 路径 `Screenshot::primary_window()` 在本机
//! macOS Metal + wgpu29 环境读回全黑帧，含清除色——判断为平台性 bug；
//! 离屏路径渲染到自有纹理，读回内容稳定。详见 BACKLOG B-21 备注）
//!
//! 验证钩子：`PHOENIX_SHOT=1` 启动约 2 秒后自动截图一次（回归用）。

use bevy::asset::RenderAssetUsages;
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

/// 离屏截图相机标记
#[derive(Component)]
pub struct CaptureCamera;

/// 截图状态：离屏目标 + 触发标志
#[derive(Resource)]
pub struct CaptureState {
    pub image: Handle<Image>,
    pub armed: bool,
}

pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_capture_camera)
            .add_systems(Update, (screenshot_trigger, arm_capture_camera).chain());
    }
}

/// 创建离屏相机（默认停用，仅在截图帧激活）。
fn setup_capture_camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let size = Extent3d {
        width: 1280,
        height: 720,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // 作为渲染目标必须有 RENDER_ATTACHMENT 用法（默认 Image 仅
    // TEXTURE_BINDING|COPY_*，直接渲染会触发 wgpu Validation Error）
    image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::COPY_SRC
        | TextureUsages::COPY_DST
        | TextureUsages::TEXTURE_BINDING;
    let handle = images.add(image);
    commands.spawn((
        Camera3d::default(),
        Camera {
            is_active: false,
            ..default()
        },
        RenderTarget::Image(ImageRenderTarget {
            handle: handle.clone(),
            scale_factor: 1.0,
        }),
        CaptureCamera,
        Name::new("CaptureCamera"),
    ));
    commands.insert_resource(CaptureState {
        image: handle,
        armed: false,
    });
}

/// F2 / 环境变量 → 置武装标志（截图帧）。
fn screenshot_trigger(
    keys: Res<ButtonInput<KeyCode>>,
    mut capture: ResMut<CaptureState>,
    mut frames: Local<u32>,
    mut env_checked: Local<bool>,
    mut env_enabled: Local<bool>,
    mut env_shot_fired: Local<bool>,
) {
    if !*env_checked {
        *env_checked = true;
        *env_enabled = std::env::var("PHOENIX_SHOT")
            .map(|v| v == "1")
            .unwrap_or(false);
    }
    *frames += 1;
    let auto_shot = *env_enabled && !*env_shot_fired && *frames >= 120;
    if auto_shot {
        *env_shot_fired = true;
    }
    if keys.just_pressed(KeyCode::F2) || auto_shot {
        capture.armed = true;
    }
}

/// 武装帧：同步主相机视角、激活离屏相机、发送截图命令；次帧停用。
fn arm_capture_camera(
    mut capture: ResMut<CaptureState>,
    mut capture_cam: Query<(&mut Camera, &mut Transform), With<CaptureCamera>>,
    main_cam: Query<&Transform, (With<Camera3d>, Without<CaptureCamera>)>,
    mut commands: Commands,
    mut prev_armed: Local<bool>,
) {
    // 每帧同步视角（截图与玩家所见一致）
    if let (Ok(mut cap), Ok(main)) = (capture_cam.single_mut(), main_cam.single()) {
        *cap.1 = *main;
    }

    let armed = capture.armed;
    if armed && !*prev_armed {
        // 武装帧：激活离屏相机并发送截图命令
        if let Ok(mut cap) = capture_cam.single_mut() {
            cap.0.is_active = true;
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("saves");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            error!("创建截图目录失败: {e}");
            capture.armed = false;
            *prev_armed = false;
            return;
        }
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("screenshot_{unix}.png"));
        commands
            .spawn(Screenshot::image(capture.image.clone()))
            .observe(save_to_disk(path));
        info!("📷 离屏截图命令已发送（saves/screenshot_{unix}.png）");
    } else if !armed && *prev_armed {
        // 截图帧已过：停用离屏相机
        if let Ok(mut cap) = capture_cam.single_mut() {
            cap.0.is_active = false;
        }
    }
    *prev_armed = armed;
    if armed {
        capture.armed = false; // 一次性
    }
}

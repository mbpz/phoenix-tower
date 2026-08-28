//! 程序化建筑网格（B-07 形制升级）：以代码生成低模，替代占位立方体。
//!
//! 目标：让塔的剪影接近真实黄鹤楼——圆柱（真实为 72 根圆柱）、
//! 攒尖顶（曲线收尖 + 檐口）、坡屋面、宝顶尖、飞檐翘角。
//! 全部低面数（每件 ≤ 500 三角面，PRD §4.5），按积木尺寸参数化生成。
//!
//! 说明：这是"精致积木风"的程序化近似，非考古级精确（真实形制需 glTF 美术）。

use bevy::asset::RenderAssetUsages;
use bevy::math::primitives::Cuboid;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};

/// 基础网格构造器。
fn new_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn finish(
    mut mesh: Mesh,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
) -> Mesh {
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// 圆柱（柱身，含柱础收放）：radius 半径、height 总高、segments 边数。
pub fn column(radius: f32, height: f32, segments: usize) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // 柱础：底部放大的矮环
    let base_r = radius * 1.35;
    let base_h = (height * 0.12).max(0.1);
    // 柱身顶环
    let top_r = radius * 0.92;

    // 用 4 段环：底础(0) / 础顶(1) / 柱顶(2) / 顶面中心(3)
    let rings: [(f32, f32); 4] = [
        (base_r, 0.0),    // 底面外缘
        (base_r, base_h), // 础顶外缘
        (top_r, height),  // 柱顶外缘
        (0.0, height),    // 顶面中心
    ];
    for (_, (r, y)) in rings.iter().enumerate() {
        for i in 0..segments {
            let a = std::f32::consts::TAU * i as f32 / segments as f32;
            positions.push([r * a.cos(), *y, r * a.sin()]);
            normals.push([a.cos(), 0.0, a.sin()]);
            uvs.push([i as f32 / segments as f32, y / height.max(0.001)]);
        }
    }
    // 顶面中心顶点
    let center_idx = rings.len() as u32 * segments as u32;
    positions.push([0.0, height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 1.0]);

    // 侧面（每环相邻两段 + 顶部收拢到中心）
    for ring in 0..(rings.len() - 1) {
        for i in 0..segments {
            let a = (ring * segments + i) as u32;
            let b = (ring * segments + (i + 1) % segments) as u32;
            let c = a + segments as u32;
            let d = b + segments as u32;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    // 顶面（最后一环 → 中心）
    let top_ring = (rings.len() - 1) as u32 * segments as u32;
    for i in 0..segments {
        let a = top_ring + i as u32;
        let b = top_ring + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[a, b, center_idx]);
    }
    // 底面封底（中心略下方，法线朝下）
    let bottom_center = center_idx + 1;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.0]);
    for i in 0..segments {
        let a = i as u32;
        let b = ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[b, a, bottom_center]);
    }

    finish(new_mesh(), positions, normals, uvs, indices)
}

/// 攒尖顶：八角形基座 + 内凹曲线收尖（黄鹤楼标志性轮廓）。
/// base_w/base_d 为基座占位尺寸（含檐口外挑），height 为总高（含宝顶尖部）。
pub fn conical_roof(base_w: f32, base_d: f32, height: f32, segments: usize) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // 基座八角（檐口外挑 10%）
    let rx = base_w * 0.5 * 1.18;
    let rz = base_d * 0.5 * 1.18;
    // 中间收腰环（内凹曲线感）：半径收至 55%，高度 45%
    let mid_y = height * 0.45;
    let mid_rx = rx * 0.55;
    let mid_rz = rz * 0.55;
    // 顶点（攒尖）
    let apex = [0.0, height, 0.0];

    // 环 0 = 基座檐口，环 1 = 收腰，环 2 = 顶点
    let ring0_base = 0u32;
    let ring1_base = segments as u32;
    let apex_idx = 2 * segments as u32;

    for i in 0..segments {
        let a = std::f32::consts::TAU * i as f32 / segments as f32;
        // 基座（外凸檐口）
        positions.push([rx * a.cos(), 0.0, rz * a.sin()]);
        normals.push([a.cos(), 0.35, a.sin()]);
        uvs.push([i as f32 / segments as f32, 0.0]);
        // 收腰
        positions.push([mid_rx * a.cos(), mid_y, mid_rz * a.sin()]);
        normals.push([a.cos(), 0.8, a.sin()]);
        uvs.push([i as f32 / segments as f32, 0.5]);
    }
    positions.push(apex);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 1.0]);

    // 基座→收腰
    for i in 0..segments {
        let a = ring0_base + i as u32;
        let b = ring0_base + ((i + 1) % segments) as u32;
        let c = ring1_base + i as u32;
        let d = ring1_base + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[a, c, b, b, c, d]);
    }
    // 收腰→顶点
    for i in 0..segments {
        let a = ring1_base + i as u32;
        let b = ring1_base + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[a, b, apex_idx]);
    }
    // 底面封底
    let bottom = apex_idx + 1;
    positions.push([0.0, -0.05, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.0]);
    for i in 0..segments {
        let a = ring0_base + i as u32;
        let b = ring0_base + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[b, a, bottom]);
    }

    finish(new_mesh(), positions, normals, uvs, indices)
}

/// 坡屋面小块（琉璃瓦）：四坡收顶的截锥体。
pub fn sloped_tile(w: f32, d: f32, pitch: f32) -> Mesh {
    let top_w = w * 0.45;
    let top_d = d * 0.45;
    let h = (w * 0.5 + d * 0.5) * 0.5 * pitch;
    let pts: Vec<[f32; 3]> = vec![
        [-w / 2.0, 0.0, -d / 2.0],
        [w / 2.0, 0.0, -d / 2.0],
        [w / 2.0, 0.0, d / 2.0],
        [-w / 2.0, 0.0, d / 2.0],
        [-top_w / 2.0, h, -top_d / 2.0],
        [top_w / 2.0, h, -top_d / 2.0],
        [top_w / 2.0, h, top_d / 2.0],
        [-top_w / 2.0, h, top_d / 2.0],
    ];
    let indices: Vec<u32> = vec![
        0, 4, 1, 1, 4, 5, 1, 5, 2, 2, 5, 6, 2, 6, 3, 3, 6, 7, 3, 7, 0, 0, 7, 4, // 侧面
        4, 6, 5, 4, 7, 6, // 顶
        0, 1, 2, 0, 2, 3, // 底
    ];
    finish(
        new_mesh(),
        pts.clone(),
        vec![[0.0, 1.0, 0.0]; pts.len()],
        vec![[0.0, 0.0]; pts.len()],
        indices,
    )
}

/// 宝顶：小型多棱尖塔（收束攒尖顶）。

/// 斗拱：三层交替 45° 出挑的承托构件（中式木构精髓）。
pub fn dougong_bracket() -> Mesh {
    // 三层：逐层加宽 + 交替旋转
    let layers: [(f32, f32, f32); 3] = [
        (0.8, 0.35, 0.0),  // 底层
        (1.0, 0.30, 45.0), // 中层（45° 交错）
        (1.25, 0.30, 0.0), // 顶层出挑
    ];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut y = 0.0f32;
    for (size, h, rot) in layers {
        let rad = rot.to_radians();
        let (c, sn) = (rad.cos(), rad.sin());
        let half = size / 2.0;
        let corners: [[f32; 3]; 8] = [
            [-half, y, -half],
            [half, y, -half],
            [half, y, half],
            [-half, y, half],
            [-half, y + h, -half],
            [half, y + h, -half],
            [half, y + h, half],
            [-half, y + h, half],
        ];
        let base = positions.len() as u32;
        for p in corners {
            // 绕 Y 轴旋转
            let x = p[0] * c - p[2] * sn;
            let z = p[0] * sn + p[2] * c;
            positions.push([x, p[1], z]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.0, 0.0]);
        }
        // 12 三角面
        indices.extend_from_slice(&[
            base,
            base + 1,
            base + 2,
            base,
            base + 2,
            base + 3, // 底
            base + 4,
            base + 6,
            base + 5,
            base + 4,
            base + 7,
            base + 6, // 顶
            base,
            base + 4,
            base + 5,
            base,
            base + 5,
            base + 1,
            base + 1,
            base + 5,
            base + 6,
            base + 1,
            base + 6,
            base + 2,
            base + 2,
            base + 6,
            base + 7,
            base + 2,
            base + 7,
            base + 3,
            base + 3,
            base + 7,
            base + 4,
            base + 3,
            base + 4,
            base,
        ]);
        y += h;
    }
    finish(new_mesh(), positions, normals, uvs, indices)
}


/// 匾额：横置薄板（0.9 宽 × 0.3 高 × 0.08 厚），字由 Text3d 子实体呈现。
pub fn plaque() -> Mesh {
    Mesh::from(Cuboid::new(0.9, 0.3, 0.08))
}

/// 琉璃瓦勾缝纹理：32×32 RGBA8，金色瓦面 + 深色勾缝 + 轻微噪声。
pub fn tile_texture(seed: u32) -> bevy::image::Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let size = 32u32;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let mut rng = seed as u64;
    let mut next = || {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng >> 33) as u32
    };
    for y in 0..size {
        for x in 0..size {
            // 瓦行间隔：每 8 行一条勾缝（深色）；竖缝每 8 列
            let grout = (x % 8 == 7) || (y % 8 == 7);
            if grout {
                data.extend_from_slice(&[58, 44, 20, 255]); // 深勾缝
            } else {
                let n = (next() % 24) as i32 - 12; // 轻微明暗噪声
                let r = (200 + n).clamp(0, 255) as u8;
                let g = (160 + n).clamp(0, 255) as u8;
                let b = (84 + n).clamp(0, 255) as u8;
                data.extend_from_slice(&[r, g, b, 255]); // 金琉璃
            }
        }
    }
    bevy::image::Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

pub fn spire(height: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let segments = 8usize;
    let base_r = 0.32;
    let mid_y = height * 0.3;
    let mid_r = 0.28;
    // 底面环 + 鼓腹环 + 顶点
    for i in 0..segments {
        let a = std::f32::consts::TAU * i as f32 / segments as f32;
        positions.push([base_r * a.cos(), 0.0, base_r * a.sin()]);
    }
    for i in 0..segments {
        let a = std::f32::consts::TAU * i as f32 / segments as f32;
        positions.push([mid_r * a.cos(), mid_y, mid_r * a.sin()]);
    }
    let apex = (2 * segments) as u32;
    positions.push([0.0, height, 0.0]);
    for i in 0..segments {
        let a = i as u32;
        let b = ((i + 1) % segments) as u32;
        let c = (segments + i) as u32;
        let d = (segments + (i + 1) % segments) as u32;
        indices.extend_from_slice(&[a, c, b, b, c, d]);
        indices.extend_from_slice(&[c, apex, d]);
    }
    // 底面
    let bottom = apex + 1;
    positions.push([0.0, -0.02, 0.0]);
    for i in 0..segments {
        let a = i as u32;
        let b = ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[b, a, bottom]);
    }
    let n = positions.len();
    finish(
        new_mesh(),
        positions,
        vec![[0.0, 1.0, 0.0]; n],
        vec![[0.0, 0.0]; n],
        indices,
    )
}

/// 檐板（楼板级）：主体楼板 + 四边出挑檐口 + 四角起翘。
/// 让每一层的楼板线读作"飞檐"，形成五层重檐的轮廓（黄鹤楼标志）。
pub fn eave_slab(w: f32, d: f32, thick: f32, chamfer: f32) -> Mesh {
    let o = 0.32; // 檐口出挑量（五层飞檐剪影）
    let u = 0.24; // 四角起翘量
    let mid_y = thick * 0.85; // 檐边（非角）略低于顶面
    let corner_y = thick + u; // 四角翘起

    let hw = w / 2.0;
    let hd = d / 2.0;
    let cw = hw - chamfer; // 切角后的角点半宽
    let cd = hd - chamfer;

    // 八角周界 8 点（顺序：角0,边0,角1,边1,角2,边2,角3,边3）
    let perim: [[f32; 2]; 8] = [
        [cw, cd],
        [hw, 0.0],
        [cw, -cd],
        [0.0, -hd],
        [-cw, -cd],
        [-hw, 0.0],
        [-cw, cd],
        [0.0, hd],
    ];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // 环 0：底面（y=0）
    for p in &perim {
        positions.push([p[0], 0.0, p[1]]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([0.0, 0.0]);
    }
    // 环 1：顶面（y=thick）
    for p in &perim {
        positions.push([p[0], thick, p[1]]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.0, 0.0]);
    }
    // 环 2：檐口（外扩 o；角点起翘、边点略低）
    for (i, p) in perim.iter().enumerate() {
        let sx = p[0].signum() * (p[0].abs() + o);
        let sz = p[1].signum() * (p[1].abs() + o);
        let y = if i % 2 == 0 { corner_y } else { mid_y };
        positions.push([sx, y, sz]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.0, 0.0]);
    }

    // 侧面：底面(环0) → 顶面(环1)
    for i in 0..8 {
        let a = i;
        let b = (i + 1) % 8;
        indices.extend_from_slice(&[a, b, 8 + b, 8 + b, 8 + a, a]);
    }
    // 檐口：顶面(环1) → 檐口(环2)
    for i in 0..8 {
        let a = 8 + i;
        let b = 8 + (i + 1) % 8;
        let c = 16 + i;
        let d = 16 + (i + 1) % 8;
        indices.extend_from_slice(&[a, b, d, d, c, a]);
    }
    // 底面封底（环 0 扇形）
    for i in 1..7 {
        indices.extend_from_slice(&[0, i, i + 1]);
    }

    finish(new_mesh(), positions, normals, uvs, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn meshes_have_valid_geometry() {
        for mesh in [
            column(0.5, 3.0, 12),
            conical_roof(4.0, 4.0, 4.0, 8),
            sloped_tile(2.0, 2.0, 0.3),
            spire(1.0),
        ] {
            let v = mesh.count_vertices();
            assert!(v > 0, "网格应包含顶点");
            let tris = mesh.triangles().map(|it| it.count()).unwrap_or(0);
            assert!(tris > 0, "网格应包含三角形");
            assert!(tris * 3 <= 2000, "低面数要求");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
            assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        }
    }

    #[test]
    fn eave_slab_has_upturned_corners() {
        let mesh = eave_slab(4.0, 4.0, 0.5, 0.9);
        assert!(mesh.count_vertices() > 0);
        let tris = mesh.triangles().map(|it| it.count()).unwrap_or(0);
        assert!(tris > 0);
        // 最高顶点应高于板厚（四角起翘）
        let pos = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let max_y = match pos {
            VertexAttributeValues::Float32x3(v) => v.iter().map(|p| p[1]).fold(f32::MIN, f32::max),
            _ => 0.0,
        };
        assert!(max_y > 0.6, "檐角应起翘（高于板厚 0.5），实际 {max_y}");
    }

    #[test]
    fn conical_roof_apex_is_peak() {
        let mesh = conical_roof(4.0, 4.0, 4.0, 8);
        let pos = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let max_y = match pos {
            VertexAttributeValues::Float32x3(v) => v.iter().map(|p| p[1]).fold(f32::MIN, f32::max),
            _ => 0.0,
        };
        assert!(
            (max_y - 4.0).abs() < 0.01,
            "攒尖顶顶点应为最高点，实际 {max_y}"
        );
    }
}

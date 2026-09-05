//! Small, shared-mesh scenery outside the editable footprint; no colliders.
use crate::riverside::RiversideMode;
use bevy::prelude::*;

pub fn setup(
    mode: Res<RiversideMode>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !mode.0 {
        return;
    }
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.57, 0.62, 0.58),
        perceptual_roughness: 0.9,
        ..default()
    });
    let pale = materials.add(StandardMaterial {
        base_color: Color::srgb(0.77, 0.76, 0.64),
        perceptual_roughness: 0.85,
        ..default()
    });
    let grass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.40, 0.29),
        ..default()
    });
    let bark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.18, 0.12),
        ..default()
    });
    let foliage = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.34, 0.27),
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.70, 0.53, 0.20),
        ..default()
    });
    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.39, 0.43),
        perceptual_roughness: 0.28,
        metallic: 0.2,
        ..default()
    });
    let ripple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.61, 0.59),
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let rock = meshes.add(Sphere::new(1.0).mesh().ico(1).expect("one subdivision"));
    let trunk = meshes.add(Cylinder::new(0.16, 1.0));
    let island = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(12));
    let mut put = |name: &'static str,
                   mesh: &Handle<Mesh>,
                   mat: &Handle<StandardMaterial>,
                   at: Vec3,
                   scale: Vec3| {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(at).with_scale(scale),
            Name::new(name),
        ));
    };
    put(
        "Yangtze • jade water",
        &cube,
        &water,
        Vec3::new(0.0, -1.1, -20.0),
        Vec3::new(260.0, 0.1, 260.0),
    );
    put(
        "Stone island",
        &island,
        &stone,
        Vec3::new(0.0, -0.7, 0.0),
        Vec3::new(11.5, 1.2, 9.0),
    );
    put(
        "Moss bank",
        &island,
        &grass,
        Vec3::new(0.0, -0.15, 0.0),
        Vec3::new(11.1, 0.2, 8.6),
    );
    put(
        "Courtyard",
        &cube,
        &pale,
        Vec3::new(0.0, -0.12, 0.0),
        Vec3::new(11.0, 0.24, 11.0),
    );
    // Subtle paving seams keep the scale readable without covering build cells.
    for i in -5..=5 {
        put(
            "Paving seam",
            &cube,
            &stone,
            Vec3::new(i as f32, 0.004, 0.0),
            Vec3::new(0.018, 0.008, 11.0),
        );
        put(
            "Paving seam",
            &cube,
            &stone,
            Vec3::new(0.0, 0.004, i as f32),
            Vec3::new(11.0, 0.008, 0.018),
        );
    }
    for i in 0..3 {
        put(
            "River steps",
            &cube,
            &pale,
            Vec3::new(0.0, -0.16 - i as f32 * 0.22, 5.8 + i as f32 * 0.55),
            Vec3::new(3.0, 0.3, 0.7),
        );
    }
    for (x, z, height) in [(-7.0, -3.0, 4.5), (-8.0, 1.5, 3.3), (7.0, -2.0, 3.8)] {
        put(
            "Pine trunk",
            &trunk,
            &bark,
            Vec3::new(x, height * 0.5, z),
            Vec3::new(1.0, height, 1.0),
        );
        for tier in 0..3 {
            let t = tier as f32;
            put(
                "Pine canopy",
                &rock,
                &foliage,
                Vec3::new(x + 0.25 * t, height - 0.6 + t * 0.7, z),
                Vec3::new(2.1 - t * 0.35, 0.65, 1.5 - t * 0.15),
            );
        }
    }
    for (x, z) in [(-6.5, 5.2), (7.2, 4.3), (6.8, -5.5), (-5.6, -6.0)] {
        put(
            "Bank rock",
            &rock,
            &stone,
            Vec3::new(x, 0.25, z),
            Vec3::new(1.1, 0.75, 0.85),
        );
        put(
            "Golden ground cover",
            &rock,
            &gold,
            Vec3::new(x + 0.7, 0.12, z + 0.6),
            Vec3::new(0.8, 0.3, 0.55),
        );
    }
    for i in 0..24 {
        let x = (i * 17 % 43) as f32 - 21.0;
        let z = -12.0 - (i * 7 % 23) as f32;
        put(
            "Water glint",
            &cube,
            &ripple,
            Vec3::new(x, -1.035, z),
            Vec3::new(1.0 + (i % 4) as f32, 0.008, 0.035),
        );
    }
    for (x, z, w, h, color) in [
        (-35.0, -65.0, 25.0, 17.0, (0.48, 0.62, 0.61)),
        (-9.0, -75.0, 23.0, 22.0, (0.53, 0.65, 0.63)),
        (22.0, -69.0, 29.0, 18.0, (0.49, 0.62, 0.61)),
        (-27.0, -42.0, 17.0, 11.0, (0.29, 0.46, 0.43)),
        (22.0, -47.0, 20.0, 13.0, (0.34, 0.50, 0.47)),
    ] {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(color.0, color.1, color.2),
            ..default()
        });
        put(
            "Distant river hills",
            &rock,
            &material,
            Vec3::new(x, -2.0, z),
            Vec3::new(w, h * 0.65, 10.0),
        );
    }
    put(
        "Distant skiff",
        &cube,
        &bark,
        Vec3::new(-10.0, -0.8, -17.0),
        Vec3::new(3.0, 0.4, 0.85),
    );
    put(
        "Skiff mast",
        &cube,
        &bark,
        Vec3::new(-10.0, 0.65, -17.0),
        Vec3::new(0.065, 2.8, 0.065),
    );
    put(
        "Linen sail",
        &cube,
        &pale,
        Vec3::new(-9.4, 1.0, -17.0),
        Vec3::new(1.2, 1.6, 0.04),
    );
}

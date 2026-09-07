//! A bounded architectural courtyard. Static shared geometry, no terrain collider.
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
    let mut material = |color: Color, roughness: f32| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: roughness,
            fog_enabled: false,
            ..default()
        })
    };
    let stone = material(Color::srgb(0.36, 0.43, 0.41), 0.92);
    let pale = material(Color::srgb(0.71, 0.72, 0.63), 0.9);
    let tile = material(Color::srgb(0.60, 0.65, 0.60), 0.88);
    let earth = material(Color::srgb(0.17, 0.25, 0.23), 0.95);
    let grass = material(Color::srgb(0.28, 0.39, 0.25), 0.95);
    let bark = material(Color::srgb(0.25, 0.13, 0.085), 0.8);
    let foliage = material(Color::srgb(0.14, 0.30, 0.24), 0.95);
    let leaf_light = material(Color::srgb(0.27, 0.43, 0.29), 0.95);
    let red = material(Color::srgb(0.40, 0.10, 0.065), 0.58);
    let water = material(Color::srgb(0.12, 0.29, 0.31), 0.32);
    let ripple = material(Color::srgb(0.25, 0.44, 0.43), 0.4);
    materials
        .get_mut(&water)
        .expect("water material")
        .fog_enabled = true;
    let lamp = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.70, 0.36),
        emissive: LinearRgba::new(1.3, 0.65, 0.19, 0.0),
        perceptual_roughness: 0.72,
        fog_enabled: false,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let rock = meshes.add(Sphere::new(1.0).mesh().ico(1).expect("one subdivision"));
    let crown = meshes.add(Sphere::new(1.0).mesh().ico(2).expect("two subdivisions"));
    let trunk = meshes.add(Cylinder::new(0.16, 1.0).mesh().resolution(8));
    let island = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(48));
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
        "Quiet water",
        &cube,
        &water,
        Vec3::new(0.0, -1.04, 0.0),
        Vec3::new(64.0, 0.1, 64.0),
    );
    put(
        "Island foundation",
        &island,
        &earth,
        Vec3::new(0.0, -0.76, 0.0),
        Vec3::new(11.8, 0.64, 10.0),
    );
    put(
        "Dressed stone edge",
        &island,
        &stone,
        Vec3::new(0.0, -0.40, 0.0),
        Vec3::new(11.6, 0.18, 9.8),
    );
    put(
        "Garden bank",
        &island,
        &grass,
        Vec3::new(0.0, -0.26, 0.0),
        Vec3::new(11.35, 0.18, 9.55),
    );
    put(
        "Courtyard grout bed",
        &cube,
        &stone,
        Vec3::new(0.0, -0.12, 0.0),
        Vec3::new(12.0, 0.22, 12.0),
    );
    // Large slabs rather than a high-contrast construction grid. The middle is
    // below the editable platform; all raised environment stays outside 7x7.
    for x in -4..4 {
        for z in -4..4 {
            put(
                "Limestone paving",
                &cube,
                if (x + 2 * z) % 4 == 0 { &tile } else { &pale },
                Vec3::new(x as f32 * 1.5 + 0.75, -0.025, z as f32 * 1.5 + 0.75),
                Vec3::new(1.47, 0.05, 1.47),
            );
        }
    }
    // Readable entry rising from the court to the real platform at y=2.
    for step in 0..5 {
        let height = (5 - step) as f32 * 0.32;
        put(
            "Platform approach",
            &cube,
            &pale,
            Vec3::new(0.0, height * 0.5, 3.72 + step as f32 * 0.43),
            Vec3::new(2.8, height, 0.43),
        );
    }
    for i in -4..=4 {
        let v = i as f32 * 1.4;
        for x in [-6.2, 6.2] {
            put(
                "Courtyard baluster",
                &cube,
                &pale,
                Vec3::new(x, 0.47, v),
                Vec3::new(0.22, 0.94, 0.22),
            );
            put(
                "Baluster cap",
                &cube,
                &stone,
                Vec3::new(x, 0.97, v),
                Vec3::new(0.30, 0.12, 0.30),
            );
        }
        put(
            "Rear baluster",
            &cube,
            &pale,
            Vec3::new(v, 0.47, -6.2),
            Vec3::new(0.22, 0.94, 0.22),
        );
    }
    for y in [0.30, 0.75] {
        for x in [-6.2, 6.2] {
            put(
                "Timber side rail",
                &cube,
                &red,
                Vec3::new(x, y, 0.0),
                Vec3::new(0.12, 0.12, 11.2),
            );
        }
        put(
            "Timber rear rail",
            &cube,
            &red,
            Vec3::new(0.0, y, -6.2),
            Vec3::new(11.2, 0.12, 0.12),
        );
    }
    // Asymmetric windswept canopies; offset clusters and visible branches,
    // not three identical stacked spheres on a pole.
    for (tree, (x, z, h)) in [(-8.0, -3.0, 4.6), (-8.5, 2.4, 3.0), (7.9, -4.0, 4.1)]
        .into_iter()
        .enumerate()
    {
        put(
            "Pine trunk",
            &trunk,
            &bark,
            Vec3::new(x, h * 0.46, z),
            Vec3::new(1.2, h, 1.2),
        );
        for cluster in 0..6 {
            let angle = cluster as f32 * 2.4 + tree as f32;
            let radius = if cluster == 5 { 0.2 } else { 1.05 };
            let cx = x + angle.cos() * radius;
            let cz = z + angle.sin() * radius;
            let cy = h - 0.25 + (cluster % 3) as f32 * 0.39;
            put(
                "Pine foliage cluster",
                &crown,
                if cluster % 3 == 0 {
                    &leaf_light
                } else {
                    &foliage
                },
                Vec3::new(cx, cy, cz),
                Vec3::new(1.15, 0.53, 0.94),
            );
            put(
                "Pine bough",
                &cube,
                &bark,
                Vec3::new((x + cx) * 0.5, cy - 0.38, (z + cz) * 0.5),
                Vec3::new((cx - x).abs() + 0.1, 0.12, (cz - z).abs() + 0.1),
            );
        }
    }
    for (i, (x, z)) in [(-7.8, 5.0), (7.6, 3.6), (7.0, -7.0), (-5.0, -7.5)]
        .into_iter()
        .enumerate()
    {
        for n in 0..3 {
            let t = n as f32;
            put(
                "Weathered bank stone",
                &rock,
                if n % 2 == 0 { &stone } else { &tile },
                Vec3::new(x + t * 0.65, 0.13 + t * 0.05, z - t * 0.42),
                Vec3::new(0.85 - t * 0.15, 0.52 - t * 0.06, 0.58),
            );
        }
        put(
            "Low garden planting",
            &crown,
            if i % 2 == 0 { &leaf_light } else { &foliage },
            Vec3::new(x - 0.3, 0.05, z + 0.7),
            Vec3::new(1.0, 0.32, 0.62),
        );
    }
    for x in [-4.5, 4.5] {
        put(
            "Garden lantern base",
            &cube,
            &stone,
            Vec3::new(x, 0.22, 5.0),
            Vec3::new(0.75, 0.44, 0.75),
        );
        put(
            "Garden lantern housing",
            &cube,
            &bark,
            Vec3::new(x, 0.77, 5.0),
            Vec3::new(0.60, 0.70, 0.60),
        );
        put(
            "Garden lantern paper",
            &cube,
            &lamp,
            Vec3::new(x, 0.79, 5.0),
            Vec3::new(0.62, 0.43, 0.62),
        );
        put(
            "Garden lantern cap",
            &cube,
            &stone,
            Vec3::new(x, 1.19, 5.0),
            Vec3::new(0.86, 0.16, 0.86),
        );
    }
    // Sparse still highlights; no continuously moving water mesh or extra camera.
    for i in 0..12 {
        let x = (i * 17 % 31) as f32 - 15.0;
        let z = -12.0 - (i * 7 % 12) as f32;
        put(
            "Water highlight",
            &cube,
            &ripple,
            Vec3::new(x, -0.982, z),
            Vec3::new(0.8 + (i % 3) as f32, 0.006, 0.025),
        );
    }
    // Two shadow-free local lights reuse the existing day/night controller.
    for x in [-4.5, 4.5] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.65, 0.30),
                intensity: 0.0,
                range: 9.0,
                radius: 0.35,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 1.15, 5.0),
            crate::building::placement::LanternLight,
            Name::new("Courtyard evening light"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courtyard_is_static_bounded_shared_and_opted_out_for_legacy_worlds() {
        for enabled in [true, false] {
            let mut app = App::new();
            app.insert_resource(RiversideMode(enabled))
                .init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<StandardMaterial>>()
                .add_systems(Startup, setup);
            app.update();
            let entities = app.world_mut().query::<&Mesh3d>().iter(app.world()).count();
            if !enabled {
                assert_eq!(entities, 0);
                continue;
            }
            assert!(
                entities > 40 && entities < 240,
                "bounded scenery, got {entities}"
            );
            assert!(app.world().resource::<Assets<Mesh>>().len() <= 7);
            assert!(app.world().resource::<Assets<StandardMaterial>>().len() <= 12);
            for transform in app.world_mut().query::<&Transform>().iter(app.world()) {
                assert!(
                    transform.scale.max_element() <= 64.0,
                    "no giant backdrop primitives"
                );
            }
            app.update();
            assert_eq!(
                app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
                entities
            );
        }
    }
}

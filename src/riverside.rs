//! Default editable courtyard. No separate world, save format, or physics rules.

use bevy::prelude::*;

use crate::building::{
    block_defs::BlockLibrary,
    blueprint::{blueprint_from_def, Blueprint, BlueprintLibrary},
    placement::{
        footprint_cells, spawn_block_entity, BlockRenderAssets, PlacedBlock, PlacedBlocks,
        PlacedRecord,
    },
    tutorial::Tutorial,
};
use crate::camera::orbit_camera::OrbitCamera;

#[derive(Resource, Default)]
pub struct RiversideMode(pub bool);

impl RiversideMode {
    pub fn requested(legacy_tower: bool, loading: bool, stress: bool) -> Self {
        // Explicit automation/import modes take priority over demonstration data.
        Self(!legacy_tower && !loading && !stress)
    }
}

pub fn model_path(id: &str) -> Option<&'static str> {
    match id {
        "datiji" => Some("models/riverside/datiji.glb"),
        "hongzhu4" => Some("models/riverside/hongzhu4.glb"),
        "liangfang5" => Some("models/riverside/liangfang5.glb"),
        "louban5" => Some("models/riverside/louban5.glb"),
        "dougong" => Some("models/riverside/dougong.glb"),
        "jiangting_roof" => Some("models/riverside/jiangting_roof.glb"),
        _ => None,
    }
}

// Anchors, not mesh transforms: all placement paths retain the same footprint.
pub(crate) const SAMPLE: [(&str, [i32; 3]); 8] = [
    ("datiji", [-3, 0, -3]),
    ("hongzhu4", [-2, 2, -2]),
    ("hongzhu4", [2, 2, -2]),
    ("hongzhu4", [-2, 2, 2]),
    ("hongzhu4", [2, 2, 2]),
    ("liangfang5", [-2, 6, -2]),
    ("liangfang5", [-2, 6, 2]),
    ("jiangting_roof", [-3, 7, -3]),
];

pub struct RiversidePlugin;
impl Plugin for RiversidePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, seed_sample).add_systems(
            Update,
            (spawn_assembly_pulses, animate_assembly_pulses).chain(),
        );
    }
}

fn seed_sample(
    mode: Res<RiversideMode>,
    mut commands: Commands,
    mut stack: ResMut<PlacedBlocks>,
    mut library: ResMut<BlockLibrary>,
    render: Res<BlockRenderAssets>,
    mut blueprint: ResMut<Blueprint>,
    mut themes: ResMut<BlueprintLibrary>,
    mut tutorial: ResMut<Tutorial>,
    mut orbit: ResMut<OrbitCamera>,
) {
    if !mode.0 || !stack.records.is_empty() {
        return;
    }
    for (id, cell) in SAMPLE {
        let anchor = IVec3::from_array(cell);
        let def = &library.defs[library.by_id[id]];
        let entity = spawn_block_entity(&mut commands, &library, &render, id, anchor, 0);
        stack.place(PlacedRecord {
            entity,
            def_id: id.into(),
            anchor,
            rot: 0,
            cells: footprint_cells(anchor, def, 0),
        });
    }
    let index = themes
        .select_by_id("riverside")
        .expect("bundled riverside blueprint");
    *blueprint = blueprint_from_def(&themes.defs[index]);
    // Free build on entry: undo the roof, then M exposes its exact snap target.
    blueprint.completion = 1.0;
    blueprint.completed = true;
    tutorial.active = false;
    library.current = library.by_id["jiangting_roof"];
    orbit.target = Vec3::new(0.0, 3.5, 0.0);
    orbit.yaw = 0.68;
    orbit.pitch = 0.40;
    orbit.distance = 28.0;
    info!(
        "RIVERSIDE_READY blocks={} editable=true",
        stack.records.len()
    );
}

#[derive(Component)]
struct AssemblyPulse {
    age: f32,
}

fn spawn_assembly_pulses(
    mode: Res<RiversideMode>,
    mut commands: Commands,
    added: Query<&Transform, Added<PlacedBlock>>,
    mut assets: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !mode.0 || added.is_empty() {
        return;
    }
    let (mesh, material) = assets.get_or_insert_with(|| {
        (
            meshes.add(Torus::new(0.94, 1.0)),
            materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.73, 0.25),
                unlit: true,
                ..default()
            }),
        )
    });
    // Bounded even on bulk imports. These are independent visuals, never colliders.
    for transform in added.iter().take(16) {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(transform.translation),
            AssemblyPulse { age: 0.0 },
            Name::new("Assembly confirmation"),
        ));
    }
}

fn animate_assembly_pulses(
    time: Res<Time>,
    mut commands: Commands,
    mut pulses: Query<(Entity, &mut AssemblyPulse, &mut Transform)>,
) {
    for (entity, mut pulse, mut transform) in &mut pulses {
        pulse.age += time.delta_secs();
        if pulse.age >= 0.45 {
            commands.entity(entity).despawn();
        } else {
            let t = pulse.age / 0.45;
            transform.scale = Vec3::new(0.5 + t, 1.0 - t, 0.5 + t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::{block_defs::load_block_library, blueprint::load_blueprint_library};

    fn sample_app() -> App {
        let mut app = App::new();
        let library = load_block_library();
        let (blueprint, themes) = load_blueprint_library();
        let render = BlockRenderAssets {
            per_def: library
                .defs
                .iter()
                .map(|d| (d.id.clone(), (Handle::default(), Handle::default())))
                .collect(),
            ghost_material: Handle::default(),
            ghost_bad_material: Handle::default(),
            blueprint_materials: Default::default(),
            blueprint_unit_mesh: Handle::default(),
        };
        app.insert_resource(RiversideMode(true))
            .insert_resource(library)
            .insert_resource(blueprint)
            .insert_resource(themes)
            .insert_resource(render)
            .insert_resource(crate::building::tutorial::load_tutorial())
            .insert_resource(OrbitCamera::default())
            .init_resource::<PlacedBlocks>()
            .add_systems(Update, seed_sample);
        app
    }

    #[test]
    fn seeding_creates_real_entities_once_and_does_not_overwrite_a_world() {
        let mut app = sample_app();
        app.update();
        let revision = app.world().resource::<PlacedBlocks>().revision;
        let entities: Vec<_> = app
            .world()
            .resource::<PlacedBlocks>()
            .records
            .iter()
            .map(|r| r.entity)
            .collect();
        assert_eq!(entities.len(), 8);
        for e in &entities {
            assert!(app.world().entity(*e).contains::<PlacedBlock>());
        }
        app.update();
        let stack = app.world().resource::<PlacedBlocks>();
        assert_eq!(stack.records.len(), 8);
        assert_eq!(stack.revision, revision);
        assert_eq!(
            stack.records.iter().map(|r| r.entity).collect::<Vec<_>>(),
            entities
        );
        assert!(!app.world().resource::<Tutorial>().active);
        assert_eq!(app.world().resource::<Blueprint>().def.id, "riverside");
        assert_eq!(
            app.world().resource::<BlockLibrary>().current_def().id,
            "jiangting_roof"
        );

        let mut app = sample_app();
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<PlacedBlocks>()
            .place(PlacedRecord {
                entity,
                def_id: "taiji".into(),
                anchor: IVec3::ZERO,
                rot: 0,
                cells: vec![IVec3::ZERO],
            });
        app.update();
        assert_eq!(app.world().resource::<PlacedBlocks>().records.len(), 1);
        assert_eq!(
            app.world().resource::<PlacedBlocks>().records[0].entity,
            entity
        );
        assert!(app.world().resource::<Tutorial>().active);
    }

    #[test]
    fn assembly_feedback_is_bounded_expires_and_never_transforms_blocks() {
        let mut app = App::new();
        app.insert_resource(RiversideMode(true))
            .insert_resource(Time::<()>::default())
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(
                Update,
                (spawn_assembly_pulses, animate_assembly_pulses).chain(),
            );
        let transform = Transform::from_xyz(2.0, 4.0, 6.0);
        let blocks: Vec<_> = (0..20)
            .map(|_| app.world_mut().spawn((PlacedBlock, transform)).id())
            .collect();
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<AssemblyPulse>>()
                .iter(app.world())
                .count(),
            16
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<AssemblyPulse>>()
                .iter(app.world())
                .count(),
            0
        );
        for e in blocks {
            assert_eq!(
                *app.world().entity(e).get::<Transform>().unwrap(),
                transform
            );
        }
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 1);
    }

    #[test]
    fn default_courtyard_preserves_explicit_tower_load_and_stress_entries() {
        assert!(RiversideMode::requested(false, false, false).0);
        for (show, load, stress) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (false, true, true),
            (true, true, true),
        ] {
            assert!(!RiversideMode::requested(show, load, stress).0);
        }
    }

    #[test]
    fn sample_cells_are_disjoint_and_match_bundled_blueprint() {
        let library = load_block_library();
        let (_, themes) = load_blueprint_library();
        let bp = blueprint_from_def(themes.defs.iter().find(|d| d.id == "riverside").unwrap());
        let mut cells = std::collections::HashMap::new();
        for (id, anchor) in SAMPLE {
            let def = &library.defs[library.by_id[id]];
            for cell in footprint_cells(IVec3::from_array(anchor), def, 0) {
                assert!(
                    cells.insert(cell, id.to_string()).is_none(),
                    "overlapping sample cell {cell}"
                );
            }
        }
        assert_eq!(cells, bp.expected);
    }

    #[test]
    fn sample_uses_normal_undo_redo_and_save_roundtrip() {
        let library = load_block_library();
        let (blueprint, _) = load_blueprint_library();
        let mut world = World::new();
        let mut stack = PlacedBlocks::default();
        for (id, anchor) in SAMPLE {
            let anchor = IVec3::from_array(anchor);
            stack.place(PlacedRecord {
                entity: world.spawn_empty().id(),
                def_id: id.into(),
                anchor,
                rot: 0,
                cells: footprint_cells(anchor, &library.defs[library.by_id[id]], 0),
            });
        }
        assert_eq!(stack.records.len(), 8);
        assert_eq!(stack.undo().unwrap().def_id, "jiangting_roof");
        assert_eq!(stack.records.len(), 7);
        assert!(stack.redo(world.spawn_empty().id()));
        let save = crate::save::build_save(&stack, &blueprint, "free");
        let bytes = crate::save::encode_bincode(&save).unwrap();
        assert_eq!(crate::save::decode_bincode(&bytes).unwrap(), save);
        assert_eq!(save.blocks.len(), 8);
    }
}

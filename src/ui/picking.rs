//! App-owned adapter for the locked bevy_lunex 0.7 picking backend.
//! Upstream chooses the first target-matching camera before checking projection,
//! so a preceding 3D camera prevents all UI hits. Only our dedicated source
//! camera may pick Lunex nodes. Keep hit geometry/visibility/blocking/depth in
//! sync with bevy_lunex's picking.rs when upgrading that dependency.
use bevy::camera::RenderTarget;
use bevy::math::FloatExt;
use bevy::picking::backend::prelude::*;
use bevy::picking::{backend::PointerHits, Pickable};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_lunex::{Dimension, NoLunexPicking, UiLayout, UiLayoutRoot, UiSourceCamera};

pub(super) struct SourceCameraPickingPlugin;

impl Plugin for SourceCameraPickingPlugin {
    fn build(&self, app: &mut App) {
        // Lunex installs its backend inside UiLunexPlugin, not as a group entry.
        // Exclude our nodes from that backend before any UI entities are spawned.
        app.register_required_components_with::<UiLayoutRoot, Pickable>(|| Pickable::IGNORE)
            .register_required_components_with::<UiLayout, NoLunexPicking>(|| NoLunexPicking)
            .register_required_components_with::<UiLayoutRoot, NoLunexPicking>(|| NoLunexPicking)
            .add_message::<PointerHits>()
            .add_systems(
                PreUpdate,
                source_camera_picking.in_set(PickingSystems::Backend),
            );
    }
}

/// Checks if any Dimension entities are under a pointer
#[allow(clippy::type_complexity)] // Keep the ECS hit-test fields and filters together.
fn source_camera_picking(
    pointers: Query<(&PointerId, &PointerLocation)>,
    cameras: Query<
        (
            Entity,
            &Camera,
            &RenderTarget,
            &GlobalTransform,
            &Projection,
        ),
        With<UiSourceCamera<0>>,
    >,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    lunex_query: Query<
        (
            Entity,
            &Dimension,
            &GlobalTransform,
            Option<&Pickable>,
            &ViewVisibility,
        ),
        Or<(With<UiLayout>, With<UiLayoutRoot>)>,
    >,
    mut output: MessageWriter<PointerHits>,
) {
    let mut sorted_nodes: Vec<_> = lunex_query
        .iter()
        .filter_map(|(entity, dimension, transform, pickable, vis)| {
            if !transform.affine().is_nan() && vis.get() {
                Some((entity, dimension, transform, pickable))
            } else {
                None
            }
        })
        .collect();

    // Stable front-to-back ordering, matching Lunex without another dependency.
    sorted_nodes.sort_by(|a, b| b.2.translation().z.total_cmp(&a.2.translation().z));

    let primary_window = primary_window.single().ok();

    for (pointer, location) in pointers.iter().filter_map(|(pointer, pointer_location)| {
        pointer_location.location().map(|loc| (pointer, loc))
    }) {
        let mut blocked = false;
        let Some((cam_entity, camera, _, cam_transform, Projection::Orthographic(cam_ortho))) =
            cameras
                .iter()
                .filter(|(_, camera, _, _, projection)| {
                    camera.is_active && matches!(projection, Projection::Orthographic(_))
                })
                .find(|(_, _, target, _, _)| {
                    target
                        .normalize(primary_window)
                        .is_some_and(|x| x == location.target)
                })
        else {
            continue;
        };

        let viewport_pos = camera
            .logical_viewport_rect()
            .map(|v| v.min)
            .unwrap_or_default();
        let pos_in_viewport = location.position - viewport_pos;

        let Ok(cursor_ray_world) = camera.viewport_to_world(cam_transform, pos_in_viewport) else {
            continue;
        };
        let cursor_ray_len = cam_ortho.far - cam_ortho.near;
        let cursor_ray_end = cursor_ray_world.origin + cursor_ray_world.direction * cursor_ray_len;

        let picks: Vec<(Entity, HitData)> = sorted_nodes
            .iter()
            .copied()
            .filter_map(|(entity, dimension, node_transform, pickable)| {
                if blocked {
                    return None;
                }

                // Transform cursor line segment to node coordinate system
                let world_to_node = node_transform.affine().inverse();
                let cursor_start_node = world_to_node.transform_point3(cursor_ray_world.origin);
                let cursor_end_node = world_to_node.transform_point3(cursor_ray_end);

                // Find where the cursor segment intersects the plane Z=0 (which is the node's
                // plane in node-local space). It may not intersect if, for example, we're
                // viewing the node side-on
                if cursor_start_node.z == cursor_end_node.z {
                    // Cursor ray is parallel to the node and misses it
                    return None;
                }
                let lerp_factor = f32::inverse_lerp(cursor_start_node.z, cursor_end_node.z, 0.0);
                if !(0.0..=1.0).contains(&lerp_factor) {
                    // Lerp factor is out of range, meaning that while an infinite line cast by
                    // the cursor would intersect the node, the node is not between the
                    // camera's near and far planes
                    return None;
                }
                // Otherwise we can interpolate the xy of the start and end positions by the
                // lerp factor to get the cursor position in node space!
                let cursor_pos_sprite = cursor_start_node.lerp(cursor_end_node, lerp_factor).xy();

                let rect = Rect::from_center_size(Vec2::ZERO, **dimension);
                let is_cursor_in_sprite = rect.contains(cursor_pos_sprite);

                blocked =
                    is_cursor_in_sprite && pickable.map(|p| p.should_block_lower).unwrap_or(true);

                is_cursor_in_sprite.then(|| {
                    let hit_pos_world =
                        node_transform.transform_point(cursor_pos_sprite.extend(0.0));
                    // Transform point from world to camera space to get the Z distance
                    let hit_pos_cam = cam_transform
                        .affine()
                        .inverse()
                        .transform_point3(hit_pos_world);
                    // HitData requires a depth as calculated from the camera's near clipping plane
                    let depth = -cam_ortho.near - hit_pos_cam.z;
                    (
                        entity,
                        HitData::new(
                            cam_entity,
                            depth,
                            Some(hit_pos_world),
                            Some(*node_transform.back()),
                        ),
                    )
                })
            })
            .collect();

        let order = camera.order as f32;
        output.write(PointerHits::new(*pointer, picks, order));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo};
    use bevy::picking::pointer::Location;

    fn picking_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(SourceCameraPickingPlugin);
        let window = app.world_mut().spawn(PrimaryWindow).id();
        let target = RenderTarget::default();
        app.world_mut().spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target: target.normalize(Some(window)).unwrap(),
                position: Vec2::splat(100.0),
            }),
        ));
        // Reproduce production spawn order: same-window perspective camera first.
        app.world_mut().spawn((
            Camera::default(),
            target.clone(),
            GlobalTransform::default(),
            Projection::Perspective(default()),
        ));
        let mut projection = OrthographicProjection::default_2d();
        projection.update(200.0, 200.0);
        let camera = Camera {
            order: 1,
            computed: ComputedCameraValues {
                clip_from_view: projection.get_clip_from_view(),
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::splat(200),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        };
        // Even an earlier untagged orthographic camera is not our UI source.
        app.world_mut().spawn((
            camera.clone(),
            target.clone(),
            GlobalTransform::default(),
            Projection::Orthographic(projection.clone()),
        ));
        let ui_camera = app
            .world_mut()
            .spawn((
                camera,
                target,
                GlobalTransform::from_translation(Vec3::Z * 1000.0),
                Projection::Orthographic(projection),
                UiSourceCamera::<0>,
            ))
            .id();
        (app, ui_camera)
    }

    fn node(app: &mut App, z: f32, visible: bool, pickable: Pickable) -> Entity {
        app.world_mut()
            .spawn((
                UiLayout::window().pack(),
                Dimension(Vec2::splat(100.0)),
                GlobalTransform::from_translation(Vec3::Z * z),
                if visible {
                    ViewVisibility::VISIBLE
                } else {
                    ViewVisibility::HIDDEN
                },
                pickable,
            ))
            .id()
    }

    fn hits(app: &mut App) -> Vec<PointerHits> {
        app.update();
        app.world_mut()
            .resource_mut::<Messages<PointerHits>>()
            .drain()
            .collect()
    }

    #[test]
    fn lunex_picking_uses_source_orthographic_after_perspective_camera() {
        let (mut app, camera) = picking_app();
        let button = node(&mut app, 10.0, true, Pickable::default());
        assert!(app.world().get::<NoLunexPicking>(button).is_some());
        let root = app.world_mut().spawn(UiLayoutRoot::new_2d()).id();
        assert!(app.world().get::<NoLunexPicking>(root).is_some());
        assert!(app
            .world()
            .get::<Pickable>(root)
            .is_some_and(|p| !p.should_block_lower && !p.is_hoverable));
        let hits = hits(&mut app);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pointer, PointerId::Mouse);
        assert_eq!(hits[0].order, 1.0);
        assert_eq!(hits[0].picks.len(), 1);
        let (entity, hit) = &hits[0].picks[0];
        assert_eq!(*entity, button);
        assert_eq!(hit.camera, camera);
        let Projection::Orthographic(projection) = app.world().get::<Projection>(camera).unwrap()
        else {
            panic!("expected orthographic");
        };
        assert!((hit.depth - (990.0 - projection.near)).abs() < 0.01);
        assert!(hit.position.unwrap().distance(Vec3::Z * 10.0) < 0.01);
        assert_eq!(hit.normal, Some(Vec3::Z));
    }

    #[test]
    fn lunex_picking_preserves_visibility_and_front_to_back_blocking() {
        let (mut app, _) = picking_app();
        let back = node(&mut app, 0.0, true, Pickable::default());
        let front = node(&mut app, 10.0, true, Pickable::default());
        node(&mut app, 20.0, false, Pickable::default());
        let first = hits(&mut app);
        assert_eq!(
            first[0].picks.iter().map(|p| p.0).collect::<Vec<_>>(),
            vec![front]
        );
        app.world_mut().entity_mut(front).insert(Pickable::IGNORE);
        let second = hits(&mut app);
        // Like upstream, emit ignored hits; the picking core handles hoverability.
        assert_eq!(
            second[0].picks.iter().map(|p| p.0).collect::<Vec<_>>(),
            vec![front, back]
        );
        assert!(second[0].picks[0].1.depth < second[0].picks[1].1.depth);
    }

    #[test]
    fn lunex_picking_excludes_owned_nodes_from_nested_upstream_backend() {
        let (mut app, _) = picking_app();
        app.add_plugins(bevy_lunex::UiLunexPickingPlugin);
        // With only the source camera, upstream would otherwise duplicate hits.
        let other_cameras: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, (With<Camera>, Without<UiSourceCamera<0>>)>()
            .iter(app.world())
            .collect();
        for camera in other_cameras {
            app.world_mut().despawn(camera);
        }
        let button = node(&mut app, 10.0, true, Pickable::default());
        let batches = hits(&mut app);
        assert_eq!(batches.len(), 2);
        assert_eq!(
            batches
                .iter()
                .filter(|batch| batch.picks.is_empty())
                .count(),
            1
        );
        let picked: Vec<_> = batches
            .iter()
            .flat_map(|batch| batch.picks.iter().map(|p| p.0))
            .collect();
        assert_eq!(picked, vec![button]);
    }

    #[test]
    fn lunex_picking_does_not_fall_back_when_source_camera_is_invalid() {
        let (mut app, camera) = picking_app();
        node(&mut app, 0.0, true, Pickable::default());
        app.world_mut().get_mut::<Camera>(camera).unwrap().is_active = false;
        assert!(hits(&mut app).is_empty());
        app.world_mut().get_mut::<Camera>(camera).unwrap().is_active = true;
        app.world_mut()
            .entity_mut(camera)
            .insert(Projection::Perspective(default()));
        assert!(hits(&mut app).is_empty());
    }
}

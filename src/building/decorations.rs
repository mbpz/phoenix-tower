//! 积木装饰子实体：每个父积木只处理一次，销毁父实体时自动级联清理。

use bevy::prelude::*;
use bevy_rich_text3d::{Text3d, Text3dStyling, TextAlign, TextAnchor, TextAtlas};

use super::placement::{BlockId, PlacedBlock};
use crate::stress::StressBlock;

/// 灯笼光源子实体，供昼夜系统调节强度。
#[derive(Component)]
pub struct LanternLight;

/// 匾额文字子实体，供 UI 诊断查询。
#[derive(Component)]
pub struct PlaqueText;

/// 标记必须放在父积木上，而不是被查询排除的子实体上。
/// 使用 archetype 排除已处理积木，稳定帧不再全量扫描建筑。
#[derive(Component)]
pub(crate) struct DecorationsReady;

type UndecoratedBlocks<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static BlockId),
    (
        With<PlacedBlock>,
        Without<DecorationsReady>,
        Without<StressBlock>,
    ),
>;

pub(crate) fn attach_block_decorations(
    mut commands: Commands,
    mut material: Local<Option<Handle<StandardMaterial>>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    blocks: UndecoratedBlocks<'_, '_>,
) {
    for (entity, id) in &blocks {
        match id.0.as_str() {
            "denglong" => {
                commands.entity(entity).with_children(|parent| {
                    parent.spawn((
                        PointLight {
                            intensity: 0.0, // 昼夜系统驱动
                            color: Color::srgb(1.0, 0.72, 0.45),
                            range: 4.0,
                            ..default()
                        },
                        LanternLight,
                    ));
                });
            }
            "biane" => {
                let material = material.get_or_insert_with(|| {
                    materials.add(StandardMaterial {
                        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    })
                });
                commands.entity(entity).with_children(|parent| {
                    parent.spawn((
                        Text3d::new("黄鹤楼"),
                        Text3dStyling {
                            size: 0.55,
                            font: "Noto Sans CJK SC".into(),
                            color: bevy::color::Srgba::new(0.95, 0.85, 0.40, 1.0),
                            align: TextAlign::Center,
                            anchor: TextAnchor::CENTER,
                            ..default()
                        },
                        Mesh3d::default(),
                        MeshMaterial3d(material.clone()),
                        Transform::from_xyz(0.0, 0.0, 0.05),
                        PlaqueText,
                    ));
                });
            }
            _ => {}
        }
        commands.entity(entity).insert(DecorationsReady);
    }
}

#[cfg(test)]
mod tests {
    use super::super::world::PlacedBlocks;
    use super::*;

    // Headless lifecycle tests: no renderer, window or GPU required.
    fn decoration_app() -> App {
        let mut app = App::new();
        app.init_resource::<PlacedBlocks>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, attach_block_decorations);
        app
    }

    #[test]
    fn new_blocks_are_decorated_without_a_revision_dependency() {
        let mut app = decoration_app();
        app.update();
        let parent = app
            .world_mut()
            .spawn((PlacedBlock, BlockId("denglong".into())))
            .id();
        app.update();
        assert_eq!(app.world().get::<Children>(parent).unwrap().len(), 1);
        assert_eq!(app.world().resource::<PlacedBlocks>().revision, 0);
    }

    #[test]
    fn decorations_are_not_duplicated_by_world_changes() {
        let mut app = decoration_app();
        let lantern = app
            .world_mut()
            .spawn((PlacedBlock, BlockId("denglong".into())))
            .id();
        let plaque = app
            .world_mut()
            .spawn((PlacedBlock, BlockId("biane".into())))
            .id();
        for revision in 1..=3 {
            app.world_mut().resource_mut::<PlacedBlocks>().revision = revision;
            app.update();
        }
        assert_eq!(app.world().get::<Children>(lantern).unwrap().len(), 1);
        assert_eq!(app.world().get::<Children>(plaque).unwrap().len(), 1);
    }

    #[test]
    fn decorations_follow_parent_lifecycle_and_skip_stress_blocks() {
        let mut app = decoration_app();
        let parent = app
            .world_mut()
            .spawn((PlacedBlock, BlockId("denglong".into())))
            .id();
        let stress = app
            .world_mut()
            .spawn((
                PlacedBlock,
                BlockId("denglong".into()),
                crate::stress::StressBlock,
            ))
            .id();
        app.world_mut().resource_mut::<PlacedBlocks>().revision = 1;
        app.update();
        let child = app.world().get::<Children>(parent).unwrap()[0];
        assert!(app.world().get::<Children>(stress).is_none());
        app.world_mut().despawn(parent);
        assert!(app.world().get_entity(child).is_err());
        let replacement = app
            .world_mut()
            .spawn((PlacedBlock, BlockId("denglong".into())))
            .id();
        app.world_mut().resource_mut::<PlacedBlocks>().revision = 2;
        app.update();
        assert_eq!(app.world().get::<Children>(replacement).unwrap().len(), 1);
    }
}

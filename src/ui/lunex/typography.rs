//! Application-owned guard for Lunex text measurement changes (including Retina).
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy_lunex::prelude::*;

/// Text is measured by Lunex. `full()` is for boxes, not labels: its Rl(100)
/// units survive set_width/set_height and get ADDED to the measured font size.
pub(super) fn centered_label_layout() -> UiLayout {
    UiLayout::window()
        .pos((Rl(50.0), Rl(50.0)))
        .anchor(Anchor::CENTER)
        .pack()
}

pub(super) fn sync_text_scale(
    mut texts: Query<(&mut Transform, &Dimension, &TextLayoutInfo), With<UiTextSize>>,
) {
    // Run after Lunex: it only reacts to Changed<Dimension>, although font
    // measurement may change independently. Both axes must fit the layout box.
    for (mut transform, dimension, measured) in &mut texts {
        if !measured.size.is_finite()
            || measured.size.min_element() <= 0.0
            || !dimension.is_finite()
            || dimension.min_element() <= 0.0
        {
            continue;
        }
        let scale = (**dimension / measured.size).min_element();
        let value = Vec3::new(scale, scale, transform.scale.z);
        if transform.scale != value {
            transform.scale = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_fits_both_axes_not_just_the_width() {
        let mut app = App::new();
        app.add_systems(Update, sync_text_scale);
        let e = app
            .world_mut()
            .spawn((
                Transform::default(),
                Dimension(Vec2::new(100.0, 20.0)),
                TextLayoutInfo {
                    size: Vec2::new(50.0, 40.0),
                    ..default()
                },
                UiTextSize::from(Ab(20.0)),
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Transform>(e).unwrap().scale.x, 0.5);
        // Font/Retina measurement can change without a different layout box.
        app.world_mut().get_mut::<TextLayoutInfo>(e).unwrap().size = Vec2::new(100.0, 80.0);
        app.update();
        assert_eq!(app.world().get::<Transform>(e).unwrap().scale.x, 0.25);
    }
    #[test]
    fn zero_or_invalid_measurement_never_produces_invalid_transform() {
        let mut app = App::new();
        app.add_systems(Update, sync_text_scale);
        let e = app
            .world_mut()
            .spawn((
                Transform::default(),
                Dimension(Vec2::ONE),
                TextLayoutInfo {
                    size: Vec2::new(0.0, 10.0),
                    ..default()
                },
                UiTextSize::from(Ab(20.0)),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Transform>(e).unwrap().scale.is_finite());
    }
}

#[cfg(test)]
mod layout_regression {
    use super::*;
    #[test]
    fn text_measurement_must_replace_full_parent_units() {
        let mut app = App::new();
        app.add_observer(|_: On<bevy_lunex::RecomputeUiLayout>| {});
        app.add_systems(Update, bevy_lunex::system_text_size_to_layout);
        let e = app
            .world_mut()
            .spawn((
                Text2d::new("积木"),
                TextLayoutInfo {
                    size: Vec2::new(36.0, 22.0),
                    ..default()
                },
                UiTextSize::from(Ab(18.0)),
                centered_label_layout(),
            ))
            .id();
        app.update();
        let layout = app.world().get::<UiLayout>(e).unwrap();
        let Some(bevy_lunex::UiLayoutType::Window(window)) = layout.layouts.get(&UiBase::id())
        else {
            panic!()
        };
        // A measured label must not retain the parent's 100% width/height.
        let expected: bevy_lunex::UiValue<Vec2> = (Ab(18.0 * 36.0 / 22.0), Ab(18.0)).into();
        assert_eq!(window.size, expected);
    }
}

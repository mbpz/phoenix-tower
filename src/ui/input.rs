//! Shared input ownership; UI picking is resolved before gameplay reads input.
use super::lunex::{LunexTab, LunexTabId, PathInput, PathInputBox};
use bevy::picking::{hover::HoverMap, pointer::PointerId, PickingSystems};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_lunex::UiLayout;

pub struct InputOwnershipPlugin;

impl Plugin for InputOwnershipPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(super::picking::SourceCameraPickingPlugin)
            .init_resource::<InputOwnership>()
            .add_systems(
                PreUpdate,
                update_ownership
                    .after(PickingSystems::Last)
                    .after(bevy::input::InputSystems),
            );
    }
}

/// Snapshot shared by camera, placement and hotkeys for the whole Update schedule.
#[derive(Resource, Default)]
pub struct InputOwnership {
    keyboard_captured: bool,
    over_ui: bool,
    left: PointerGesture,
    right: PointerGesture,
    world_click: bool,
}

/// Logical pixels; latch a drag once crossed, even when it returns to its origin.
const CLICK_DRAG_THRESHOLD: f32 = 10.0;

#[derive(Default)]
struct PointerGesture {
    origin: Option<Vec2>,
    active: bool,
    started_over_ui: bool,
    dragged: bool,
}

impl PointerGesture {
    fn update(
        &mut self,
        mouse: &ButtonInput<MouseButton>,
        button: MouseButton,
        cursor: Option<Vec2>,
        over_ui: bool,
    ) -> bool {
        if mouse.just_pressed(button) {
            *self = Self {
                origin: cursor,
                active: true,
                started_over_ui: over_ui,
                dragged: false,
            };
        }
        if !mouse.pressed(button) && !mouse.just_released(button) {
            *self = Self::default();
        }
        if self.active {
            self.dragged |= match (self.origin, cursor) {
                (Some(a), Some(b)) => a.distance(b) > CLICK_DRAG_THRESHOLD,
                _ => true,
            };
        }
        mouse.just_released(button)
            && self.active
            && !self.started_over_ui
            && !over_ui
            && !self.dragged
    }

    fn ui_captured(&self) -> bool {
        self.active && self.started_over_ui
    }
}

impl InputOwnership {
    fn update_pointer(
        &mut self,
        mouse: &ButtonInput<MouseButton>,
        cursor: Option<Vec2>,
        over_ui: bool,
    ) {
        self.over_ui = over_ui;
        self.world_click = self.left.update(mouse, MouseButton::Left, cursor, over_ui);
        self.right
            .update(mouse, MouseButton::Right, cursor, over_ui);
    }

    pub fn world_click(&self) -> bool {
        self.world_click
    }

    pub fn orbit_allowed(&self) -> bool {
        self.left.active && !self.left.started_over_ui && self.left.dragged
    }

    pub fn pan_allowed(&self) -> bool {
        self.right.active && !self.right.started_over_ui
    }

    pub fn zoom_allowed(&self) -> bool {
        !self.over_ui && !self.left.ui_captured() && !self.right.ui_captured()
    }
}

/// Optional ownership keeps isolated/headless gameplay-system tests independent of UI.
pub(crate) fn keyboard_allowed(ownership: Option<&InputOwnership>) -> bool {
    ownership.is_none_or(|input| !input.keyboard_captured)
}

pub(crate) fn shortcuts_allowed(
    keys: &ButtonInput<KeyCode>,
    ownership: Option<&InputOwnership>,
) -> bool {
    keyboard_allowed(ownership)
        && !keys.any_pressed([
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
            KeyCode::ShiftLeft,
            KeyCode::ShiftRight,
        ])
}

fn update_ownership(
    mut ownership: ResMut<InputOwnership>,
    mut path: Option<ResMut<PathInput>>,
    tab: Option<Res<LunexTab>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    hover_map: Option<Res<HoverMap>>,
    ui_nodes: Query<Entity, With<UiLayout>>,
    path_boxes: Query<(), With<PathInputBox>>,
    parents: Query<&ChildOf>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    // Release text focus before taking the frame's keyboard-ownership snapshot.
    // Text/decoration hits may target descendants rather than the input box itself.
    if mouse
        .as_ref()
        .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left))
    {
        let over_path = hover_map.as_ref().is_some_and(|map| {
            map.get(&PointerId::Mouse).is_some_and(|hits| {
                hits.keys().any(|entity| {
                    path_boxes.contains(*entity)
                        || parents
                            .iter_ancestors(*entity)
                            .any(|ancestor| path_boxes.contains(ancestor))
                })
            })
        });
        if !over_path {
            if let Some(path) = path.as_mut() {
                path.focused = false;
            }
        }
    }
    ownership.keyboard_captured =
        path.is_some_and(|path| path.focused) && tab.is_some_and(|tab| tab.0 == LunexTabId::Saves);
    let over_ui = hover_map.is_some_and(|map| {
        map.get(&PointerId::Mouse)
            .is_some_and(|hits| hits.keys().any(|entity| ui_nodes.contains(*entity)))
    });
    let window = windows.single().ok();
    let cursor = window
        .filter(|window| window.focused)
        .and_then(Window::cursor_position);
    if let Some(mouse) = mouse {
        ownership.update_pointer(&mouse, cursor, over_ui);
        if window.is_some_and(|window| !window.focused) {
            ownership.left = PointerGesture::default();
            ownership.right = PointerGesture::default();
            ownership.world_click = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        input: &mut InputOwnership,
        mouse: &ButtonInput<MouseButton>,
        pos: Vec2,
        over_ui: bool,
    ) {
        input.update_pointer(mouse, Some(pos), over_ui);
    }

    #[test]
    fn ui_origin_is_captured_until_release_and_blocks_camera() {
        let mut input = InputOwnership::default();
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::ZERO, true);
        mouse.clear();
        frame(&mut input, &mouse, Vec2::new(100.0, 0.0), false);
        assert!(!input.orbit_allowed());
        assert!(!input.zoom_allowed());
        mouse.release(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::new(2.0, 0.0), false);
        assert!(!input.world_click());

        mouse.reset_all();
        mouse.press(MouseButton::Right);
        frame(&mut input, &mouse, Vec2::ZERO, true);
        mouse.clear();
        frame(&mut input, &mouse, Vec2::X * 100.0, false);
        assert!(!input.pan_allowed());
    }

    #[test]
    fn short_ui_origin_click_cannot_leak_across_panel_edge() {
        let mut input = InputOwnership::default();
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::ZERO, true);
        mouse.clear();
        mouse.release(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::X * 2.0, false);
        assert!(!input.world_click());
        assert!(
            !input.left.dragged,
            "ownership, not the drag threshold, must block this click"
        );
    }

    #[test]
    fn modified_shortcuts_are_not_gameplay() {
        let mut keys = ButtonInput::<KeyCode>::default();
        assert!(shortcuts_allowed(&keys, None));
        for modifier in [
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
            KeyCode::ShiftLeft,
            KeyCode::ShiftRight,
        ] {
            keys.reset_all();
            keys.press(modifier);
            assert!(!shortcuts_allowed(&keys, None), "{modifier:?}");
        }
    }

    #[test]
    fn out_and_back_drag_never_becomes_a_placement_click() {
        let mut input = InputOwnership::default();
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::ZERO, false);
        mouse.clear();
        frame(&mut input, &mouse, Vec2::X * 30.0, false);
        assert!(input.orbit_allowed());
        mouse.release(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::X, false);
        assert!(!input.world_click());
    }

    #[test]
    fn world_click_and_wheel_still_work_outside_ui() {
        let mut input = InputOwnership::default();
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::ZERO, false);
        assert!(!input.orbit_allowed(), "click jitter must not orbit");
        mouse.clear();
        mouse.release(MouseButton::Left);
        frame(&mut input, &mouse, Vec2::X, false);
        assert!(input.world_click());
        assert!(input.zoom_allowed());
        mouse.clear();
        frame(&mut input, &mouse, Vec2::X, true);
        assert!(!input.world_click());
        assert!(!input.zoom_allowed());
    }

    fn focused_path_app() -> App {
        let mut app = App::new();
        app.init_resource::<InputOwnership>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<HoverMap>()
            .insert_resource(PathInput {
                value: "/tmp/castle.ptw".into(),
                focused: true,
            })
            .insert_resource(LunexTab(LunexTabId::Saves))
            .add_systems(PreUpdate, update_ownership);
        app
    }

    fn hover_mouse(app: &mut App, entity: Entity) {
        use bevy::picking::backend::HitData;

        app.world_mut()
            .resource_mut::<HoverMap>()
            .entry(PointerId::Mouse)
            .or_default()
            .insert(entity, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
    }

    fn primary_press(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
    }

    #[test]
    fn primary_click_inside_path_preserves_focus() {
        let mut app = focused_path_app();
        let input_box = app.world_mut().spawn(PathInputBox).id();
        hover_mouse(&mut app, input_box);
        primary_press(&mut app);
        assert!(app.world().resource::<PathInput>().focused);
        assert!(!keyboard_allowed(Some(
            app.world().resource::<InputOwnership>()
        )));
    }

    #[test]
    fn primary_click_on_nested_path_descendant_preserves_focus() {
        let mut app = focused_path_app();
        let input_box = app.world_mut().spawn(PathInputBox).id();
        let child = app.world_mut().spawn(ChildOf(input_box)).id();
        let text = app.world_mut().spawn(ChildOf(child)).id();
        hover_mouse(&mut app, text);
        primary_press(&mut app);
        assert!(app.world().resource::<PathInput>().focused);
        assert!(!keyboard_allowed(Some(
            app.world().resource::<InputOwnership>()
        )));
    }

    #[test]
    fn primary_click_on_other_ui_blurs_path() {
        let mut app = focused_path_app();
        let panel = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((PathInputBox, ChildOf(panel)));
        let button = app
            .world_mut()
            .spawn((UiLayout::window().pack(), ChildOf(panel)))
            .id();
        hover_mouse(&mut app, button);
        primary_press(&mut app);
        assert!(!app.world().resource::<PathInput>().focused);
        let ownership = app.world().resource::<InputOwnership>();
        assert!(ownership.over_ui);
        assert!(keyboard_allowed(Some(ownership)));
    }

    #[test]
    fn primary_click_on_world_entity_blurs_path() {
        let mut app = focused_path_app();
        let block = app.world_mut().spawn_empty().id();
        hover_mouse(&mut app, block);
        primary_press(&mut app);
        assert!(!app.world().resource::<PathInput>().focused);
        assert!(keyboard_allowed(Some(
            app.world().resource::<InputOwnership>()
        )));
    }

    #[test]
    fn primary_click_on_empty_world_restores_rotation_and_history_keyboard_access() {
        let mut app = focused_path_app();
        let mut rotation_keys = ButtonInput::default();
        rotation_keys.press(KeyCode::KeyR);
        let mut undo_keys = ButtonInput::default();
        undo_keys.press(KeyCode::SuperLeft);
        undo_keys.press(KeyCode::KeyZ);
        app.update();
        let ownership = app.world().resource::<InputOwnership>();
        assert!(!shortcuts_allowed(&rotation_keys, Some(ownership)));
        assert!(!keyboard_allowed(Some(ownership)));

        primary_press(&mut app);
        let ownership = app.world().resource::<InputOwnership>();
        assert!(rotation_keys.just_pressed(KeyCode::KeyR));
        assert!(shortcuts_allowed(&rotation_keys, Some(ownership)));
        assert!(undo_keys.just_pressed(KeyCode::KeyZ));
        assert!(keyboard_allowed(Some(ownership)));
        assert!(!app.world().resource::<PathInput>().focused);
        assert_eq!(app.world().resource::<PathInput>().value, "/tmp/castle.ptw");
    }

    #[test]
    fn hover_outside_and_secondary_click_do_not_blur_path() {
        let mut app = focused_path_app();
        app.update();
        assert!(app.world().resource::<PathInput>().focused);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert!(app.world().resource::<PathInput>().focused);
        assert!(!keyboard_allowed(Some(
            app.world().resource::<InputOwnership>()
        )));
    }

    #[test]
    fn primary_click_without_hover_map_blurs_path() {
        let mut app = focused_path_app();
        app.world_mut().remove_resource::<HoverMap>();
        primary_press(&mut app);
        assert!(!app.world().resource::<PathInput>().focused);
        assert!(keyboard_allowed(Some(
            app.world().resource::<InputOwnership>()
        )));
    }

    #[test]
    fn hidden_path_focus_does_not_capture_keyboard() {
        let mut app = App::new();
        app.add_plugins(InputOwnershipPlugin)
            .insert_resource(PathInput {
                value: String::new(),
                focused: true,
            })
            .insert_resource(LunexTab(LunexTabId::Saves));
        app.update();
        assert!(!keyboard_allowed(Some(
            app.world().resource::<InputOwnership>()
        )));
        app.world_mut().resource_mut::<LunexTab>().0 = LunexTabId::Blocks;
        app.update();
        assert!(keyboard_allowed(Some(
            app.world().resource::<InputOwnership>()
        )));
    }
}

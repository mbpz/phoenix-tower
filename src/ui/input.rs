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
        // Opt-in native QA: observe delivery and ownership without logging text.
        if std::env::var("PHOENIX_INPUT_PROBE").as_deref() == Ok("1") {
            app.add_systems(Last, log_input_probe);
        }
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
    motion: Option<(Vec2, Vec2)>,
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

    pub(crate) fn frame_motion(&self) -> Option<(Vec2, Vec2)> {
        self.motion
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

/// A quick chord may be pressed and released between rendered frames.
/// Keep modifiers for that frame so history works and plain hotkeys cannot leak.
pub(crate) fn modifier_active(keys: &ButtonInput<KeyCode>, key: KeyCode) -> bool {
    keys.pressed(key) || keys.just_released(key)
}

pub(crate) fn shortcuts_allowed(
    keys: &ButtonInput<KeyCode>,
    ownership: Option<&InputOwnership>,
) -> bool {
    keyboard_allowed(ownership)
        && ![
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
            KeyCode::ShiftLeft,
            KeyCode::ShiftRight,
        ]
        .into_iter()
        .any(|key| modifier_active(keys, key))
}

/// Preserve event order when a press, motion and release share one render frame.
#[derive(Default)]
struct NativePointerPath {
    reader: bevy::ecs::message::MessageCursor<bevy::window::WindowEvent>,
    cursor: Option<Vec2>,
    held: ButtonInput<MouseButton>,
    enabled: bool,
}

impl NativePointerPath {
    fn update(
        &mut self,
        input: &mut InputOwnership,
        events: &Messages<bevy::window::WindowEvent>,
        window: Entity,
        fallback_ui: bool,
        hit: &super::picking::UiCursorHitTest,
    ) -> bool {
        use bevy::{input::ButtonState, window::WindowEvent};
        let events: Vec<_> = self.reader.read(events).collect();
        self.enabled |= events.iter().any(|event| {
            matches!(event,
            WindowEvent::CursorMoved(e) if e.window == window)
        }) || events
            .iter()
            .any(|event| matches!(event, WindowEvent::MouseButtonInput(e) if e.window == window));
        if !self.enabled {
            return false;
        }
        let over_ui = |cursor: Option<Vec2>| {
            cursor
                .and_then(|pos| hit.contains(window, pos))
                .unwrap_or(fallback_ui)
        };
        input.update_pointer(&self.held, self.cursor, over_ui(self.cursor));
        let mut click = false;
        let mut motion = (Vec2::ZERO, Vec2::ZERO);
        for event in events {
            match event {
                WindowEvent::CursorMoved(e) if e.window == window => {
                    let previous = self.cursor;
                    self.cursor = Some(e.position);
                    input.update_pointer(&self.held, self.cursor, over_ui(self.cursor));
                    let delta = previous.map_or(Vec2::ZERO, |p| e.position - p);
                    if self.held.pressed(MouseButton::Left) && input.orbit_allowed() {
                        motion.0 += delta;
                    }
                    if self.held.pressed(MouseButton::Right) && input.pan_allowed() {
                        motion.1 += delta;
                    }
                }
                WindowEvent::MouseButtonInput(e) if e.window == window => {
                    match e.state {
                        ButtonState::Pressed => self.held.press(e.button),
                        ButtonState::Released => self.held.release(e.button),
                    }
                    input.update_pointer(&self.held, self.cursor, over_ui(self.cursor));
                    click |= input.world_click();
                    self.held.clear();
                }
                WindowEvent::CursorLeft(e) if e.window == window => {
                    self.cursor = None;
                    input.update_pointer(&self.held, None, true);
                }
                WindowEvent::WindowFocused(e) if e.window == window && !e.focused => {
                    self.held.reset_all();
                    self.cursor = None;
                    input.left = PointerGesture::default();
                    input.right = PointerGesture::default();
                    click = false;
                    motion = (Vec2::ZERO, Vec2::ZERO);
                }
                _ => {}
            }
        }
        input.world_click = click;
        input.motion = Some(motion);
        true
    }
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
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    events: Option<Res<Messages<bevy::window::WindowEvent>>>,
    mut native: Local<NativePointerPath>,
    hit: super::picking::UiCursorHitTest,
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
    let primary = windows.single().ok();
    let window = primary.map(|(_, w)| w);
    let cursor = window
        .filter(|window| window.focused)
        .and_then(Window::cursor_position);
    if let Some(mouse) = mouse {
        ownership.motion = None;
        let native_handled = primary
            .zip(events.as_deref())
            .is_some_and(|((entity, _), events)| {
                native.update(&mut ownership, events, entity, over_ui, &hit)
            });
        if !native_handled {
            ownership.update_pointer(&mouse, cursor, over_ui);
        }
        ownership.over_ui = over_ui;
        if window.is_some_and(|window| !window.focused) {
            ownership.left = PointerGesture::default();
            ownership.right = PointerGesture::default();
            ownership.world_click = false;
            ownership.motion = Some((Vec2::ZERO, Vec2::ZERO));
            native.held.reset_all();
            native.cursor = None;
        }
    }
}

/// Diagnostic only: never mutates gameplay or records typed text/key names.
fn log_input_probe(
    input: Res<InputOwnership>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    stack: Option<Res<crate::building::placement::PlacedBlocks>>,
    camera: Option<Res<crate::camera::orbit_camera::OrbitCamera>>,
    scroll: Option<Res<bevy::input::mouse::AccumulatedMouseScroll>>,
    mut previous_focus: Local<Option<bool>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let focus_changed = *previous_focus != Some(window.focused);
    *previous_focus = Some(window.focused);
    let key_presses = keys.get_just_pressed().count();
    let key_releases = keys.get_just_released().count();
    let left_down = mouse.just_pressed(MouseButton::Left);
    let left_up = mouse.just_released(MouseButton::Left);
    let motion = input.frame_motion().unwrap_or_default();
    let scroll_delta = scroll.as_ref().map_or(Vec2::ZERO, |scroll| scroll.delta);
    if focus_changed
        || key_presses > 0
        || key_releases > 0
        || left_down
        || left_up
        || motion != (Vec2::ZERO, Vec2::ZERO)
        || scroll_delta != Vec2::ZERO
    {
        info!(
            "INPUT_PROBE focused={} keyboard_captured={} over_ui={} left_down={} left_up={} world_click={} orbit={} key_presses={} key_releases={} history_modifier={} shift={} pieces={} cursor={:?} scale={} motion={:?} scroll={:?} camera={:?}",
            window.focused, input.keyboard_captured, input.over_ui, left_down, left_up,
            input.world_click(), input.orbit_allowed(), key_presses, key_releases,
            [KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]
                .into_iter().any(|key| modifier_active(&keys, key)),
            [KeyCode::ShiftLeft, KeyCode::ShiftRight].into_iter().any(|key| modifier_active(&keys, key)),
            stack.as_ref().map_or(0, |stack| stack.records.len()), window.cursor_position(), window.scale_factor(),
            motion, scroll_delta, camera.as_ref().map(|camera| (camera.yaw, camera.pitch, camera.distance)),
        );
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
    fn released_modifier_in_same_frame_still_blocks_plain_shortcuts() {
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::SuperLeft);
        keys.press(KeyCode::KeyR);
        keys.release(KeyCode::KeyR);
        keys.release(KeyCode::SuperLeft);
        assert!(!shortcuts_allowed(&keys, None));
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

    #[test]
    fn same_frame_native_drag_is_not_a_click_even_when_it_returns_to_origin() {
        use bevy::input::{mouse::MouseButtonInput, ButtonState};
        use bevy::window::{CursorMoved, WindowEvent};
        for end in [Vec2::new(130.0, 100.0), Vec2::new(101.0, 100.0)] {
            let mut app = focused_path_app();
            app.add_message::<WindowEvent>();
            let window = app
                .world_mut()
                .spawn((Window::default(), PrimaryWindow))
                .id();
            for event in [
                WindowEvent::CursorMoved(CursorMoved {
                    window,
                    position: Vec2::splat(100.0),
                    delta: None,
                }),
                WindowEvent::MouseButtonInput(MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                }),
                WindowEvent::CursorMoved(CursorMoved {
                    window,
                    position: Vec2::new(130.0, 100.0),
                    delta: Some(Vec2::new(30.0, 0.0)),
                }),
                WindowEvent::CursorMoved(CursorMoved {
                    window,
                    position: end,
                    delta: None,
                }),
                WindowEvent::MouseButtonInput(MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                }),
            ] {
                app.world_mut().write_message(event);
            }
            app.world_mut()
                .query::<&mut Window>()
                .single_mut(app.world_mut())
                .unwrap()
                .set_cursor_position(Some(end));
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.press(MouseButton::Left);
            mouse.release(MouseButton::Left);
            app.update();
            let input = app.world().resource::<InputOwnership>();
            assert!(
                !input.world_click(),
                "A full-frame drag must not place at its final point"
            );
            assert!(
                input.left.dragged,
                "Latch the maximum excursion, not only final delta"
            );
            assert_eq!(
                input.frame_motion(),
                Some((end - Vec2::splat(100.0), Vec2::ZERO))
            );
            app.update();
            let input = app.world().resource::<InputOwnership>();
            assert!(!input.world_click());
            assert_eq!(input.frame_motion(), Some((Vec2::ZERO, Vec2::ZERO)));
        }
    }

    #[test]
    fn native_path_preserves_ui_press_origin_and_short_world_clicks() {
        use bevy::input::{mouse::MouseButtonInput, ButtonState};
        use bevy::window::{CursorMoved, WindowEvent};
        for (start, end, world_click, dragged) in [
            (100.0, 180.0, false, true),
            (149.0, 151.0, false, false),
            (180.0, 181.0, true, false),
        ] {
            let (mut app, _) = super::super::picking::tests::picking_app();
            super::super::picking::tests::node(&mut app, 10.0, true, Pickable::default());
            app.init_resource::<InputOwnership>()
                .init_resource::<ButtonInput<MouseButton>>()
                .add_message::<WindowEvent>()
                .add_systems(PreUpdate, update_ownership.after(PickingSystems::Backend));
            let window = app
                .world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(app.world())
                .unwrap();
            let mut native_window = Window::default();
            native_window.set_cursor_position(Some(Vec2::new(end, 100.0)));
            app.world_mut().entity_mut(window).insert(native_window);
            for event in [
                WindowEvent::CursorMoved(CursorMoved {
                    window,
                    position: Vec2::new(start, 100.0),
                    delta: None,
                }),
                WindowEvent::MouseButtonInput(MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                }),
                WindowEvent::CursorMoved(CursorMoved {
                    window,
                    position: Vec2::new(end, 100.0),
                    delta: None,
                }),
                WindowEvent::MouseButtonInput(MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                }),
            ] {
                app.world_mut().write_message(event);
            }
            app.update();
            let input = app.world().resource::<InputOwnership>();
            assert_eq!(input.world_click(), world_click, "{start}->{end}");
            assert_eq!(input.left.dragged, dragged);
            assert_eq!(
                input.frame_motion(),
                Some((Vec2::ZERO, Vec2::ZERO)),
                "UI-origin movement must not rotate"
            );
            app.update();
            let input = app.world().resource::<InputOwnership>();
            assert!(!input.world_click());
            assert_eq!(input.frame_motion(), Some((Vec2::ZERO, Vec2::ZERO)));
        }
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

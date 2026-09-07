//! Let the window event loop wait while idle instead of polling or busy sleeping.
use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use std::time::Duration;

pub struct PerformancePlugin;

fn idle_settings(uncapped: bool) -> WinitSettings {
    if uncapped {
        // Explicit benchmark escape hatch; presentation is still subject to VSync.
        WinitSettings::continuous()
    } else {
        WinitSettings {
            // Input/window events can wake earlier: this is an idle budget,
            // not a hard FPS cap while the player is interacting.
            focused_mode: UpdateMode::reactive_low_power(Duration::from_secs_f64(1.0 / 60.0)),
            unfocused_mode: UpdateMode::reactive_low_power(Duration::from_millis(100)),
        }
    }
}

impl Plugin for PerformancePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(idle_settings(
            std::env::var("PHOENIX_UNCAPPED").as_deref() == Ok("1"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_foreground_uses_event_loop_wait_instead_of_continuous_polling() {
        let settings = idle_settings(false);
        assert_eq!(
            settings.focused_mode,
            UpdateMode::reactive_low_power(Duration::from_secs_f64(1.0 / 60.0))
        );
    }

    #[test]
    fn background_ignores_raw_device_events_and_waits_one_hundred_ms() {
        let settings = idle_settings(false);
        assert_eq!(
            settings.unfocused_mode,
            UpdateMode::Reactive {
                wait: Duration::from_millis(100),
                react_to_device_events: false,
                react_to_user_events: true,
                react_to_window_events: true,
            }
        );
    }

    #[test]
    fn benchmark_opt_out_uses_continuous_updates_in_both_focus_states() {
        let settings = idle_settings(true);
        assert_eq!(settings.focused_mode, UpdateMode::Continuous);
        assert_eq!(settings.unfocused_mode, UpdateMode::Continuous);
    }
}

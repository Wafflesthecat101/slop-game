//! Player-tunable settings — the single source of truth for values that menus,
//! the player controller and (later) audio read from, instead of each module
//! hardcoding its own consts.
//!
//! [`Settings`] is a plain [`Resource`] inserted at startup by
//! [`SettingsPlugin`]. It derives serde so save/load can persist it later; the
//! defaults are chosen to match the game's current feel exactly (see the
//! `#[cfg(test)]` assertions below), so introducing this resource changes
//! nothing until a menu edits it.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Rendering quality preset. Kept as a small enum so menus can offer discrete
/// choices and later systems can branch on it without magic numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Quality {
    Low,
    Medium,
    #[default]
    High,
}

/// The single source of truth for user-tunable values (look sensitivity,
/// field of view, audio volumes, quality). Defaults reproduce the previous
/// hardcoded feel exactly.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Mouse-look sensitivity (radians of rotation per pixel of motion).
    pub mouse_sensitivity: f32,
    /// Base (walking) camera field of view, in radians.
    pub fov: f32,
    /// Field of view while sprinting, in radians — the wider "sense of speed".
    pub fov_run: f32,
    /// Master output volume, `0.0..=1.0`.
    pub master_volume: f32,
    /// Music volume, `0.0..=1.0`.
    pub music_volume: f32,
    /// Sound-effects volume, `0.0..=1.0`.
    pub sfx_volume: f32,
    /// Rendering quality preset.
    pub quality: Quality,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.0022,
            fov: 1.20,
            fov_run: 1.40,
            master_volume: 1.0,
            music_volume: 0.5,
            sfx_volume: 0.8,
            quality: Quality::High,
        }
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_previous_consts() {
        let s = Settings::default();
        // These were the module-level consts in `player.rs`; the resource must
        // default to them so game feel is byte-for-byte unchanged.
        assert_eq!(s.mouse_sensitivity, 0.0022);
        assert_eq!(s.fov, 1.20);
        assert_eq!(s.fov_run, 1.40);
    }

    #[test]
    fn defaults_have_sensible_volumes_and_quality() {
        let s = Settings::default();
        assert_eq!(s.master_volume, 1.0);
        assert_eq!(s.music_volume, 0.5);
        assert_eq!(s.sfx_volume, 0.8);
        assert_eq!(s.quality, Quality::High);
    }
}

//! Persistent world progress — remembering the reawakened world across
//! sessions (design pillar P2).
//!
//! On startup this plugin loads a small [`SaveData`] blob (the user
//! [`Settings`] plus the set of rekindled shrine ids) and applies it: settings
//! are restored, and every shrine whose id was saved starts already lit. It
//! then autosaves whenever progress changes (a [`ShrineLit`] message) or the
//! settings change, so the world is always remembered without an explicit
//! "save" action (P1: calm, no fuss).
//!
//! Persistence is deliberately dependency-light: a JSON string written to a
//! `save.json` file next to the working directory on native (the same cwd
//! Bevy resolves `assets/` against — see `AGENTS.md`), or to the browser's
//! `localStorage` on WASM. The two platforms share the serde (de)serialization
//! and differ only in the tiny [`load_string`]/[`save_string`] backend.
//!
//! ## Ordering
//! Shrines are spawned by [`crate::beacons`] at `Startup`. Loading settings
//! runs at `Startup` (the [`Settings`] resource already exists then), and the
//! lit-state is applied at `PostStartup` — guaranteed to run after every
//! `Startup` system, so the shrine entities are present. Applying lit-state
//! reuses the public [`beacons::light_shrine`] hook, which brightens the orb
//! and bumps [`Progress`] *without* emitting [`ShrineLit`], so a loaded world
//! replays no rekindle SFX or feedback.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::beacons::{Orb, Progress, Shrine, ShrineLit, light_shrine};
use crate::settings::Settings;

/// Everything persisted between sessions: user settings plus the ids of the
/// shrines that have been rekindled.
#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
struct SaveData {
    settings: Settings,
    lit_shrines: Vec<u32>,
}

/// The lit-shrine ids loaded at startup, applied once at `PostStartup`.
#[derive(Resource, Default)]
struct LoadedProgress {
    lit_shrines: HashSet<u32>,
}

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_settings)
            .add_systems(PostStartup, apply_loaded_progress)
            .add_systems(Update, (autosave_on_rekindle, autosave_on_settings_change));
    }
}

/// Load the save at startup: restore [`Settings`] and stash the lit-shrine ids
/// for [`apply_loaded_progress`]. Absent or malformed saves fall back to
/// defaults (see [`parse`]).
fn load_settings(mut settings: ResMut<Settings>, mut commands: Commands) {
    let data = parse(load_string());
    *settings = data.settings;
    commands.insert_resource(LoadedProgress {
        lit_shrines: data.lit_shrines.into_iter().collect(),
    });
}

/// Apply the loaded lit-state to the already-spawned shrines (runs once at
/// `PostStartup`, after `beacons` spawns them at `Startup`).
fn apply_loaded_progress(
    loaded: Res<LoadedProgress>,
    mut progress: ResMut<Progress>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shrines: Query<(&mut Shrine, &Children)>,
    orbs: Query<&MeshMaterial3d<StandardMaterial>, With<Orb>>,
    mut lights: Query<&mut PointLight>,
) {
    for (mut shrine, children) in &mut shrines {
        if loaded.lit_shrines.contains(&shrine.index) {
            light_shrine(
                &mut shrine,
                children,
                &mut progress,
                &mut materials,
                &orbs,
                &mut lights,
            );
        }
    }
}

/// Autosave when a shrine is rekindled (progress changed).
fn autosave_on_rekindle(
    mut lit: MessageReader<ShrineLit>,
    settings: Res<Settings>,
    shrines: Query<&Shrine>,
) {
    if lit.read().count() == 0 {
        return;
    }
    persist(&snapshot(&settings, &shrines));
}

/// Autosave when settings change (menu edits). `is_changed` keeps this from
/// writing every frame — only when the resource was actually touched.
fn autosave_on_settings_change(settings: Res<Settings>, shrines: Query<&Shrine>) {
    if !settings.is_changed() {
        return;
    }
    persist(&snapshot(&settings, &shrines));
}

/// Build a [`SaveData`] from the live world: current settings plus the ids of
/// every lit shrine.
fn snapshot(settings: &Settings, shrines: &Query<&Shrine>) -> SaveData {
    SaveData {
        settings: *settings,
        lit_shrines: shrines.iter().filter(|s| s.lit).map(|s| s.index).collect(),
    }
}

/// Deserialize a save string (or `None`) into [`SaveData`], falling back to
/// defaults for a missing or malformed save (never panics).
fn parse(raw: Option<String>) -> SaveData {
    match raw {
        Some(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            warn!("ignoring malformed save, starting fresh: {e}");
            SaveData::default()
        }),
        None => SaveData::default(),
    }
}

/// Serialize and write a [`SaveData`] to the platform's store.
fn persist(data: &SaveData) {
    match serde_json::to_string(data) {
        Ok(s) => save_string(&s),
        Err(e) => warn!("failed to serialize save: {e}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
const SAVE_PATH: &str = "save.json";

#[cfg(not(target_arch = "wasm32"))]
fn load_string() -> Option<String> {
    std::fs::read_to_string(SAVE_PATH).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_string(data: &str) {
    if let Err(e) = std::fs::write(SAVE_PATH, data) {
        warn!("failed to write save file: {e}");
    }
}

#[cfg(target_arch = "wasm32")]
const SAVE_KEY: &str = "lumen_save";

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn load_string() -> Option<String> {
    local_storage()?.get_item(SAVE_KEY).ok()?
}

#[cfg(target_arch = "wasm32")]
fn save_string(data: &str) {
    if let Some(storage) = local_storage()
        && let Err(e) = storage.set_item(SAVE_KEY, data)
    {
        warn!("failed to write localStorage save: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_data_round_trips_through_json() {
        let data = SaveData {
            settings: Settings {
                fov: 1.5,
                master_volume: 0.25,
                ..Default::default()
            },
            lit_shrines: vec![0, 3, 7],
        };
        let json = serde_json::to_string(&data).unwrap();
        let back: SaveData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn missing_save_uses_defaults() {
        let data = parse(None);
        assert_eq!(data, SaveData::default());
        assert!(data.lit_shrines.is_empty());
    }

    #[test]
    fn malformed_save_falls_back_to_defaults() {
        let data = parse(Some("{ not valid json ]".to_string()));
        assert_eq!(data, SaveData::default());
    }

    #[test]
    fn valid_save_string_is_loaded() {
        let original = SaveData {
            settings: Settings::default(),
            lit_shrines: vec![2, 5],
        };
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(parse(Some(json)), original);
    }
}

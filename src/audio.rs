//! Minimal audio foundation for later music/SFX/ambient work.
//!
//! Bevy's built-in audio was dropped in the 3D rewrite (see `AGENTS.md`); this
//! plugin re-enables a thin, `Settings`-aware path. It exposes two tiny helpers
//! — [`play_sfx`] (a one-shot, despawns when done) and [`play_music`] (a looping
//! track) — that scale their volume by the user's [`Settings`], so every future
//! sound honours the master/SFX/music sliders without duplicating that logic.
//!
//! As a smoke test, a single placeholder SFX plays whenever a shrine is
//! rekindled (a [`ShrineLit`] message). Audio is spawn-and-forget entities, so
//! the reacting system stays cheap (P4) and the sound itself is calm (P1).

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::beacons::ShrineLit;
use crate::settings::Settings;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_audio_assets)
            .add_systems(Update, play_rekindle_sfx);
    }
}

/// Loaded audio handles kept alive for the game's lifetime.
#[derive(Resource)]
struct AudioAssets {
    rekindle: Handle<AudioSource>,
}

fn load_audio_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(AudioAssets {
        rekindle: assets.load("audio/rekindle.wav"),
    });
}

/// Effective linear volume for a one-shot SFX: master × sfx, clamped to `0..=1`.
fn sfx_level(settings: &Settings) -> f32 {
    (settings.master_volume * settings.sfx_volume).clamp(0.0, 1.0)
}

/// Effective linear volume for a looping music track: master × music, clamped.
fn music_level(settings: &Settings) -> f32 {
    (settings.master_volume * settings.music_volume).clamp(0.0, 1.0)
}

/// Play a one-shot sound effect at the user's SFX volume. The audio entity
/// despawns itself once playback finishes.
pub fn play_sfx(commands: &mut Commands, settings: &Settings, source: Handle<AudioSource>) {
    commands.spawn((
        AudioPlayer::new(source),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(sfx_level(settings))),
    ));
}

/// Play a looping music track at the user's music volume, returning its entity
/// so a caller can stop it later by despawning it.
pub fn play_music(
    commands: &mut Commands,
    settings: &Settings,
    source: Handle<AudioSource>,
) -> Entity {
    commands
        .spawn((
            AudioPlayer::new(source),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(music_level(settings))),
        ))
        .id()
}

fn play_rekindle_sfx(
    mut lit: MessageReader<ShrineLit>,
    mut commands: Commands,
    settings: Res<Settings>,
    audio: Res<AudioAssets>,
) {
    for _ in lit.read() {
        play_sfx(&mut commands, &settings, audio.rekindle.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_products_of_master_and_channel() {
        let s = Settings::default();
        assert_eq!(sfx_level(&s), s.master_volume * s.sfx_volume);
        assert_eq!(music_level(&s), s.master_volume * s.music_volume);
    }

    #[test]
    fn levels_are_clamped_to_unit_range() {
        let over = Settings {
            master_volume: 2.0,
            sfx_volume: 2.0,
            music_volume: 2.0,
            ..Default::default()
        };
        assert_eq!(sfx_level(&over), 1.0);
        assert_eq!(music_level(&over), 1.0);

        let muted = Settings {
            master_volume: 0.0,
            ..Default::default()
        };
        assert_eq!(sfx_level(&muted), 0.0);
        assert_eq!(music_level(&muted), 0.0);
    }
}

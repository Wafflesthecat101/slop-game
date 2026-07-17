use crate::GameState;
use crate::actions::{Actions, set_movement_actions};
use crate::loading::AudioAssets;
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

pub struct InternalAudioPlugin;

/// Controls the game's audio: a looping "flying" sound that plays while the
/// player is moving during `GameState::Playing`.
///
/// The sound is explicitly stopped `OnExit(Playing)` rather than simply
/// letting the [`FlyingAudio`] resource be overwritten next time `Playing`
/// is entered: without that, restarting a round (via the game-over screen's
/// "Play Again" button) would leave the *previous* round's audio instance
/// looping forever in the background, since only the resource handle — not
/// the underlying playing sound — would be replaced.
impl Plugin for InternalAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AudioPlugin)
            .add_systems(OnEnter(GameState::Playing), start_audio)
            .add_systems(
                Update,
                control_flying_sound
                    .after(set_movement_actions)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), stop_audio);
    }
}

#[derive(Resource)]
struct FlyingAudio(Handle<AudioInstance>);

fn start_audio(mut commands: Commands, audio_assets: Res<AudioAssets>, audio: Res<Audio>) {
    audio.pause();
    let handle = audio
        .play(audio_assets.flying.clone())
        .looped()
        .with_volume(0.3)
        .handle();
    commands.insert_resource(FlyingAudio(handle));
}

fn control_flying_sound(
    actions: Res<Actions>,
    audio: Res<FlyingAudio>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    if let Some(mut instance) = audio_instances.get_mut(&audio.0) {
        match instance.state() {
            PlaybackState::Paused { .. } if actions.player_movement.is_some() => {
                instance.resume(AudioTween::default());
            }
            PlaybackState::Playing { .. } if actions.player_movement.is_none() => {
                instance.pause(AudioTween::default());
            }
            _ => {}
        }
    }
}

/// Stops the current round's audio instance and drops [`FlyingAudio`].
///
/// See the [`InternalAudioPlugin`] docs for why this must happen `OnExit`
/// rather than being left to `start_audio` to handle next round.
fn stop_audio(
    mut commands: Commands,
    audio: Option<Res<FlyingAudio>>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    let Some(audio) = audio else {
        return;
    };
    if let Some(mut instance) = audio_instances.get_mut(&audio.0) {
        instance.stop(AudioTween::default());
    }
    commands.remove_resource::<FlyingAudio>();
}

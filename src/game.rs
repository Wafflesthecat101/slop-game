use crate::GameState;
use crate::loading::TextureAssets;
use crate::movement::Velocity;
use crate::player::Player;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rand::random_range;

/// The "catch the coins" gameplay loop: this is the actual game.
///
/// While in `GameState::Playing`, [`Collectible`]s continuously spawn near
/// the top of the screen and drift downward. Moving the player sprite over
/// one collects it and increases [`Score`]; missing one lets it fall off
/// the bottom of the screen, where it is despawned for free (there's no
/// penalty for missing one — the challenge is the ever-shrinking spawn
/// interval, see [`spawn_interval_for_score`]). The round ends, and the
/// game transitions to `GameState::GameOver`, once [`RoundTimer`] elapses.
pub struct GameplayPlugin;

/// Length of a single round, in seconds.
const ROUND_DURATION_SECS: f32 = 45.0;
/// How close (in world units) the player's center must get to a
/// collectible's center to pick it up.
const COLLECT_RADIUS: f32 = 24.0;
/// Downward speed collectibles fall at, in world units per second.
const FALL_SPEED: f32 = 80.0;
/// Extra margin (in pixels) below the visible window before an off-screen
/// collectible is despawned, so it can't visibly "pop" out of existence.
const DESPAWN_MARGIN: f32 = 32.0;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .add_systems(OnEnter(GameState::Playing), setup_round)
            .add_systems(
                Update,
                (
                    spawn_collectibles,
                    collect_collectibles,
                    despawn_offscreen_collectibles,
                    tick_round_timer,
                )
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), cleanup_round);
    }
}

/// Number of collectibles the player has caught this round.
///
/// This is a persistent [`Resource`] (initialized once to `0` and never
/// removed) rather than something re-inserted every round, because both the
/// HUD (during `Playing`) and the game-over screen (after `Playing`) need
/// to read it — [`setup_round`] simply resets it back to `0` when a new
/// round starts.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score(pub u32);

/// Counts down the time remaining in the current round. Recreated fresh by
/// [`setup_round`] every time `GameState::Playing` is entered so that
/// replaying never inherits a stale/expired timer from a previous round.
#[derive(Resource)]
pub struct RoundTimer(pub Timer);

/// Controls how frequently new collectibles spawn. Its duration is
/// continuously shortened as the score increases (see
/// [`spawn_interval_for_score`]), which is what makes later parts of a
/// round more frantic than the start.
#[derive(Resource)]
struct SpawnTimer(Timer);

/// Marker for an entity the player can pick up to increase their score.
#[derive(Component)]
struct Collectible;

/// Resets round-scoped state when a new round begins.
///
/// Explicitly despawns any leftover [`Collectible`]s first: normally
/// [`cleanup_round`] already does this on `OnExit(Playing)`, but this guards
/// against ever entering `Playing` twice in a row without an intervening
/// exit (not currently possible given the state graph, but cheap insurance
/// against future state-machine changes introducing that possibility).
fn setup_round(
    mut commands: Commands,
    score: Option<ResMut<Score>>,
    collectibles: Query<Entity, With<Collectible>>,
) {
    for entity in &collectibles {
        commands.entity(entity).despawn();
    }
    if let Some(mut score) = score {
        score.0 = 0;
    }
    commands.insert_resource(RoundTimer(Timer::from_seconds(
        ROUND_DURATION_SECS,
        TimerMode::Once,
    )));
    commands.insert_resource(SpawnTimer(Timer::from_seconds(
        spawn_interval_for_score(0),
        TimerMode::Repeating,
    )));
}

/// Despawns every [`Collectible`] and removes the round-scoped timer
/// resources when leaving `Playing`, so a returning player (via the menu or
/// a new round) never sees stale coins hanging in the air.
fn cleanup_round(
    mut commands: Commands,
    collectibles: Query<Entity, With<Collectible>>,
    round_timer: Option<Res<RoundTimer>>,
    spawn_timer: Option<Res<SpawnTimer>>,
) {
    for entity in &collectibles {
        commands.entity(entity).despawn();
    }
    if round_timer.is_some() {
        commands.remove_resource::<RoundTimer>();
    }
    if spawn_timer.is_some() {
        commands.remove_resource::<SpawnTimer>();
    }
}

/// The spawn interval (in seconds) between collectibles, given the current
/// score.
///
/// A pure function so difficulty scaling can be unit-tested without an ECS
/// `World`. Interval decreases linearly with score and is clamped to
/// [`MIN_SPAWN_INTERVAL`] so the game speeds up but never becomes
/// impossible (or spawns collectibles literally on top of each other).
const MIN_SPAWN_INTERVAL: f32 = 0.35;
const BASE_SPAWN_INTERVAL: f32 = 1.2;
const SPAWN_INTERVAL_DECAY_PER_POINT: f32 = 0.03;

fn spawn_interval_for_score(score: u32) -> f32 {
    (BASE_SPAWN_INTERVAL - score as f32 * SPAWN_INTERVAL_DECAY_PER_POINT).max(MIN_SPAWN_INTERVAL)
}

/// Spawns a new [`Collectible`] at a random horizontal position just above
/// the top of the window whenever [`SpawnTimer`] elapses, then re-arms the
/// timer with a (possibly shorter) interval based on the current score.
fn spawn_collectibles(
    mut commands: Commands,
    time: Res<Time>,
    mut spawn_timer: ResMut<SpawnTimer>,
    score: Res<Score>,
    textures: Res<TextureAssets>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    spawn_timer.0.tick(time.delta());
    if !spawn_timer.0.is_finished() {
        return;
    }
    spawn_timer
        .0
        .set_duration(std::time::Duration::from_secs_f32(
            spawn_interval_for_score(score.0),
        ));

    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;
    let x = random_range(-half_width..half_width);
    let y = half_height + DESPAWN_MARGIN;

    commands.spawn((
        Sprite::from_image(textures.github.clone()),
        Transform::from_xyz(x, y, 0.5).with_scale(Vec3::splat(0.3)),
        Collectible,
        Velocity(Vec2::new(0., -FALL_SPEED)),
    ));
}

/// Despawns any [`Collectible`] that has drifted past the bottom of the
/// window without being collected.
fn despawn_offscreen_collectibles(
    mut commands: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    collectibles: Query<(Entity, &Transform), With<Collectible>>,
) {
    let despawn_y = -(window.height() / 2.0) - DESPAWN_MARGIN;
    for (entity, transform) in &collectibles {
        if transform.translation.y < despawn_y {
            commands.entity(entity).despawn();
        }
    }
}

/// Returns whether two points are within `radius` of each other.
///
/// A pure, easily-unit-tested helper factored out of [`collect_collectibles`]
/// so the collision rule itself (a simple circle check) is decoupled from
/// how the positions are obtained from the ECS.
fn is_colliding(a: Vec2, b: Vec2, radius: f32) -> bool {
    a.distance_squared(b) <= radius * radius
}

/// Awards a point and despawns the collectible for every [`Collectible`]
/// currently within [`COLLECT_RADIUS`] of the player.
fn collect_collectibles(
    mut commands: Commands,
    mut score: ResMut<Score>,
    player: Query<&Transform, With<Player>>,
    collectibles: Query<(Entity, &Transform), With<Collectible>>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    let player_pos = player_transform.translation.xy();

    for (entity, transform) in &collectibles {
        if is_colliding(player_pos, transform.translation.xy(), COLLECT_RADIUS) {
            commands.entity(entity).despawn();
            score.0 += 1;
        }
    }
}

/// Advances [`RoundTimer`] and moves the game to `GameState::GameOver` once
/// it finishes.
fn tick_round_timer(
    time: Res<Time>,
    mut round_timer: ResMut<RoundTimer>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    round_timer.0.tick(time.delta());
    if round_timer.0.is_finished() {
        next_state.set(GameState::GameOver);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_interval_decreases_with_score_but_is_clamped() {
        assert_eq!(spawn_interval_for_score(0), BASE_SPAWN_INTERVAL);
        assert!(spawn_interval_for_score(10) < spawn_interval_for_score(0));
        // At a high enough score the interval must not drop below the floor.
        assert_eq!(spawn_interval_for_score(1_000), MIN_SPAWN_INTERVAL);
    }

    #[test]
    fn spawn_interval_is_monotonically_non_increasing() {
        let mut previous = spawn_interval_for_score(0);
        for score in (1..=200).step_by(5) {
            let current = spawn_interval_for_score(score);
            assert!(current <= previous);
            previous = current;
        }
    }

    #[test]
    fn collision_detects_overlap_within_radius() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 0.0);
        assert!(is_colliding(a, b, 15.0));
        assert!(is_colliding(a, b, 10.0)); // exactly touching counts
        assert!(!is_colliding(a, b, 5.0));
    }
}

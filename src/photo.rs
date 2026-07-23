//! Photo mode — hide the HUD, freeze the world and fly a detached camera
//! around it to frame a shot, with a couple of cheap post-processing filters.
//!
//! Design fit (pillars P1/P3): the reawakened world is worth looking at, so
//! this gives players a calm, no-stakes way to compose and admire it. There
//! are no captures-to-disk or fail states — it is purely a way to *look*.
//!
//! ## How it coordinates with the state machine (owned by [`crate::states`])
//!
//! Gameplay systems (player control, beacon/orb animation, day-night,
//! reawakening) all `run_if(in_state(GameState::Playing))`, so the simplest
//! way to *freeze* the world without touching any of those modules is to leave
//! `Playing`. Photo mode therefore drives `Playing → Paused` on entry and
//! `Paused → Playing` on exit via [`NextState`] (which [`crate::states`]
//! explicitly allows). It **reuses the single existing `Camera3d`** rather than
//! spawning a second one: this keeps the boot-time `Camera3d == 1` invariant
//! (see `scripts/expectations.default.txt`) trivially true and sidesteps the
//! `Single<_, With<Camera3d>>` queries elsewhere. The camera's transform is
//! snapshotted on entry and restored on exit, so the player camera comes back
//! exactly where it was.
//!
//! The pause overlay ([`crate::menu`]) also spawns on entering `Paused`; photo
//! mode hides it (and the HUD) by driving [`Visibility`] on the root UI nodes,
//! recording their prior visibility so exit restores everything exactly. It
//! never edits `hud.rs` or `menu.rs`.
//!
//! Controls: `P` toggles photo mode (as does `Esc`, which the state machine
//! already maps to leaving `Paused`). While in photo mode: `WASD` + mouse fly
//! the camera, `Space`/`Ctrl` rise/descend, `Shift` moves faster, and `F`
//! cycles the filter.

use crate::settings::Settings;
use crate::states::GameState;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::ColorGrading;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

/// Fly speeds (m/s) for the detached camera.
const FLY_SPEED: f32 = 18.0;
const FLY_SPEED_FAST: f32 = 55.0;

pub struct PhotoPlugin;

impl Plugin for PhotoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PhotoState>()
            .add_systems(Update, toggle_photo)
            .add_systems(OnEnter(GameState::Paused), enter_photo)
            .add_systems(OnExit(GameState::Paused), exit_photo)
            .add_systems(
                Update,
                (photo_upkeep, fly_camera, apply_filter, update_overlay).run_if(in_photo),
            );
    }
}

/// The simple post filters photo mode can apply to the camera. Deliberately
/// cheap — they only nudge the existing colour-grade and bloom, adding no new
/// render passes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Filter {
    /// No grading change — the world as it looks in play.
    #[default]
    None,
    /// A warm, desaturated sepia grade for a nostalgic postcard look.
    Warm,
    /// A soft, glowy dream look via boosted bloom and a touch more colour.
    Dream,
}

impl Filter {
    /// The next filter in the cycle (wraps back to [`Filter::None`]).
    fn next(self) -> Self {
        match self {
            Filter::None => Filter::Warm,
            Filter::Warm => Filter::Dream,
            Filter::Dream => Filter::None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Filter::None => "None",
            Filter::Warm => "Warm",
            Filter::Dream => "Dream",
        }
    }
}

/// Photo-mode state and the snapshot needed to restore gameplay exactly.
#[derive(Resource, Default)]
struct PhotoState {
    /// True while the world is paused *for photo mode* (as opposed to the
    /// normal pause menu), so shared `Paused` systems can tell the two apart.
    requested: bool,
    /// True once the entry setup (snapshot + hide UI) has been applied.
    entered: bool,
    /// Free-fly look angles (radians), seeded from the camera on entry.
    yaw: f32,
    pitch: f32,
    /// Camera transform captured on entry, restored verbatim on exit.
    saved_transform: Option<Transform>,
    /// Camera bloom intensity captured on entry (a filter may change it).
    saved_bloom: f32,
    /// Prior visibility of each root UI node we hid, restored on exit.
    saved_visibility: Vec<(Entity, Visibility)>,
    filter: Filter,
}

/// Run condition: only run the photo-mode systems once entry setup is applied.
fn in_photo(state: Res<PhotoState>) -> bool {
    state.entered
}

/// Marks UI spawned by photo mode so it is excluded from the HUD-hiding sweep
/// and torn down on exit.
#[derive(Component)]
struct PhotoUi;

/// `P` toggles photo mode; `F` cycles the filter while it is active.
///
/// Entering only makes sense from active gameplay (`Playing`); exiting works
/// from the photo-paused state. `Esc` is intentionally *not* handled here —
/// the state machine already maps it to leaving `Paused`, and the
/// `OnExit(Paused)` cleanup restores everything either way.
fn toggle_photo(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut photo: ResMut<PhotoState>,
) {
    if photo.entered && keys.just_pressed(KeyCode::KeyF) {
        photo.filter = photo.filter.next();
    }

    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    match state.get() {
        GameState::Playing => {
            photo.requested = true;
            next.set(GameState::Paused);
        }
        GameState::Paused if photo.requested => {
            next.set(GameState::Playing);
        }
        _ => {}
    }
}

/// On entering `Paused` *for photo mode*, snapshot the camera, hide the HUD and
/// pause overlay, and drop into free-fly. A normal pause (menu) leaves
/// `requested` false and is untouched.
fn enter_photo(
    mut commands: Commands,
    mut photo: ResMut<PhotoState>,
    camera: Query<(Entity, &Transform, &Bloom, Has<ColorGrading>), With<Camera3d>>,
    mut roots: Query<(Entity, &mut Visibility), (With<Node>, Without<ChildOf>, Without<PhotoUi>)>,
) {
    if !photo.requested {
        return;
    }

    if let Ok((entity, transform, bloom, has_grading)) = camera.single() {
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        photo.yaw = yaw;
        photo.pitch = pitch;
        photo.saved_transform = Some(*transform);
        photo.saved_bloom = bloom.intensity;
        // The filters drive `ColorGrading`; make sure the camera carries one.
        if !has_grading {
            commands.entity(entity).insert(ColorGrading::default());
        }
    }

    // Hide every root UI node (HUD, pause overlay), remembering its prior
    // visibility so exit can restore it exactly.
    photo.saved_visibility.clear();
    for (entity, mut visibility) in &mut roots {
        photo.saved_visibility.push((entity, *visibility));
        *visibility = Visibility::Hidden;
    }

    commands.spawn((
        PhotoUi,
        Text::new(String::new()),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));

    photo.filter = Filter::None;
    photo.entered = true;
}

/// On leaving `Paused`, if photo mode was active restore the camera transform,
/// bloom and UI visibility exactly, and tear down the photo overlay.
fn exit_photo(
    mut commands: Commands,
    mut photo: ResMut<PhotoState>,
    mut camera: Query<(&mut Transform, &mut Bloom), With<Camera3d>>,
    mut visibilities: Query<&mut Visibility>,
    overlay: Query<Entity, With<PhotoUi>>,
) {
    if !photo.entered {
        return;
    }

    if let Ok((mut transform, mut bloom)) = camera.single_mut() {
        if let Some(saved) = photo.saved_transform {
            *transform = saved;
        }
        bloom.intensity = photo.saved_bloom;
    }

    for (entity, visibility) in photo.saved_visibility.drain(..) {
        if let Ok(mut current) = visibilities.get_mut(entity) {
            *current = visibility;
        }
    }

    for entity in &overlay {
        commands.entity(entity).despawn();
    }

    photo.saved_transform = None;
    photo.requested = false;
    photo.entered = false;
}

/// Keep the cursor grabbed and the HUD/pause overlay hidden every frame while
/// in photo mode. This overrides the state machine's `OnEnter(Paused)` cursor
/// release and catches the pause overlay, which spawns on the same transition.
fn photo_upkeep(
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut roots: Query<&mut Visibility, (With<Node>, Without<PhotoUi>, Without<ChildOf>)>,
) {
    if let Ok(mut cursor) = cursor.single_mut() {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
    for mut visibility in &mut roots {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Free-fly the camera: mouse steers, `WASD` moves in the view plane,
/// `Space`/`Ctrl` rise/descend, `Shift` moves faster.
fn fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    settings: Res<Settings>,
    mut photo: ResMut<PhotoState>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };

    photo.yaw -= motion.delta.x * settings.mouse_sensitivity;
    photo.pitch = (photo.pitch - motion.delta.y * settings.mouse_sensitivity).clamp(-1.54, 1.54);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, photo.yaw, photo.pitch, 0.0);

    let forward = *transform.forward();
    let right = *transform.right();
    let mut wish = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        wish += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish -= right;
    }
    if keys.pressed(KeyCode::Space) {
        wish += Vec3::Y;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        wish -= Vec3::Y;
    }

    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        FLY_SPEED_FAST
    } else {
        FLY_SPEED
    };
    transform.translation += wish.normalize_or_zero() * speed * time.delta_secs();
}

/// Apply the current filter to the camera by nudging its colour-grade and
/// bloom — no new render passes.
fn apply_filter(
    photo: Res<PhotoState>,
    mut camera: Query<(&mut Bloom, &mut ColorGrading), With<Camera3d>>,
) {
    let Ok((mut bloom, mut grading)) = camera.single_mut() else {
        return;
    };
    match photo.filter {
        Filter::None => {
            bloom.intensity = photo.saved_bloom;
            grading.global.post_saturation = 1.0;
            grading.global.temperature = 0.0;
        }
        Filter::Warm => {
            bloom.intensity = photo.saved_bloom;
            grading.global.post_saturation = 0.45;
            grading.global.temperature = 0.4;
        }
        Filter::Dream => {
            bloom.intensity = 0.6;
            grading.global.post_saturation = 1.15;
            grading.global.temperature = 0.12;
        }
    }
}

/// Keep the on-screen hint in sync with the active filter.
fn update_overlay(photo: Res<PhotoState>, mut overlay: Query<&mut Text, With<PhotoUi>>) {
    let Ok(mut text) = overlay.single_mut() else {
        return;
    };
    **text = format!(
        "PHOTO MODE  \u{2022}  WASD/Space/Ctrl fly  \u{2022}  Shift fast  \u{2022}  F filter: {}  \u{2022}  P/Esc exit",
        photo.filter.label()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_cycles_through_all_and_wraps() {
        assert_eq!(Filter::None.next(), Filter::Warm);
        assert_eq!(Filter::Warm.next(), Filter::Dream);
        assert_eq!(Filter::Dream.next(), Filter::None);
    }

    #[test]
    fn filter_labels_are_distinct() {
        let labels = [
            Filter::None.label(),
            Filter::Warm.label(),
            Filter::Dream.label(),
        ];
        assert_eq!(labels[0], "None");
        assert!(labels[1] != labels[0] && labels[2] != labels[1]);
    }

    #[test]
    fn photo_state_defaults_are_inactive() {
        let state = PhotoState::default();
        assert!(!state.requested);
        assert!(!state.entered);
        assert_eq!(state.filter, Filter::None);
        assert!(state.saved_transform.is_none());
    }
}

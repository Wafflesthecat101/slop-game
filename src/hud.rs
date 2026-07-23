//! Minimal on-screen UI: a centre crosshair, a controls hint, a subtle
//! objective counter that reflects the [`crate::beacons::Progress`] resource,
//! and a one-time "follow the light" onboarding nudge.

use crate::beacons::{Progress, ShrineLit};
use crate::states::GameState;
use bevy::prelude::*;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(
            Update,
            (update_objective, update_onboarding).run_if(in_state(GameState::Playing)),
        );
    }
}

/// Calm cyan the objective counter rests in.
const OBJ_COLOR: Srgba = Srgba::rgb(0.7, 1.0, 1.0);
/// Faint alpha when idle so the counter recedes rather than nags.
const OBJ_IDLE_ALPHA: f32 = 0.12;
/// Alpha while freshly changed, so an update gently draws the eye.
const OBJ_ACTIVE_ALPHA: f32 = 0.9;
/// Seconds the counter stays fully visible after a change.
const OBJ_HOLD_SECS: f32 = 2.5;
/// Seconds spent easing back down to idle afterwards.
const OBJ_FADE_SECS: f32 = 1.5;

/// Alpha the onboarding hint fades in to.
const HINT_ALPHA: f32 = 0.7;

/// Objective counter visibility as a function of the time since it last
/// changed: held bright for a moment, then eased (smoothstep) down to a faint
/// idle level.
fn objective_alpha(secs_since_change: f32) -> f32 {
    if secs_since_change <= OBJ_HOLD_SECS {
        OBJ_ACTIVE_ALPHA
    } else {
        let t = ((secs_since_change - OBJ_HOLD_SECS) / OBJ_FADE_SECS).clamp(0.0, 1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        OBJ_ACTIVE_ALPHA + (OBJ_IDLE_ALPHA - OBJ_ACTIVE_ALPHA) * eased
    }
}

/// Marks the objective counter text and tracks how long since it last changed
/// so `update_objective` can fade it.
#[derive(Component, Default)]
struct ObjectiveText {
    since_change: f32,
}

/// Marks the one-time onboarding hint; `dismissed` flips on the first rekindle.
#[derive(Component, Default)]
struct OnboardingHint {
    dismissed: bool,
}

fn spawn_hud(mut commands: Commands) {
    // Full-screen container that centres the crosshair.
    commands
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(6.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ));
        });

    commands.spawn((
        Text::new("WASD move  \u{2022}  Mouse look  \u{2022}  Shift run  \u{2022}  Space jump  \u{2022}  Esc free cursor"),
        TextFont {
            font_size: bevy::text::FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    // Objective counter, top-left. Populated + faded by `update_objective`.
    commands.spawn((
        ObjectiveText::default(),
        Text::new("Shrines 0"),
        TextFont {
            font_size: bevy::text::FontSize::Px(20.0),
            ..default()
        },
        TextColor(OBJ_COLOR.with_alpha(OBJ_IDLE_ALPHA).into()),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(14.0),
            ..default()
        },
    ));

    // One-time onboarding nudge, centred below the crosshair. Fades in, then
    // dismisses itself the first time the player rekindles a shrine.
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Percent(60.0),
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|row| {
            row.spawn((
                OnboardingHint::default(),
                Text::new("follow the light"),
                TextFont {
                    font_size: bevy::text::FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
            ));
        });
}

/// Keep the objective text in sync with progress and gently fade it: a change
/// (rekindle or progress update) pops it to full opacity, after which it eases
/// back to a faint idle level. Announces completion when every shrine is lit.
fn update_objective(
    time: Res<Time>,
    progress: Res<Progress>,
    mut lit: MessageReader<ShrineLit>,
    obj: Single<(&mut Text, &mut TextColor, &mut ObjectiveText)>,
) {
    let (mut text, mut color, mut state) = obj.into_inner();

    let progress_changed = progress.is_changed();
    let rekindled = !lit.is_empty();
    lit.clear();

    if progress_changed {
        **text = if progress.lit >= progress.total && progress.total > 0 {
            format!("All {} shrines rekindled!", progress.total)
        } else {
            format!("Shrines {} / {}", progress.lit, progress.total)
        };
    }

    if progress_changed || rekindled {
        state.since_change = 0.0;
    } else {
        state.since_change += time.delta_secs();
    }

    *color = TextColor(
        OBJ_COLOR
            .with_alpha(objective_alpha(state.since_change))
            .into(),
    );
}

/// Fade the onboarding hint in, then dismiss it the first time a shrine is
/// rekindled: on dismissal it eases out and despawns so it never returns.
fn update_onboarding(
    time: Res<Time>,
    mut commands: Commands,
    mut lit: MessageReader<ShrineLit>,
    mut hint: Query<(Entity, &mut TextColor, &mut OnboardingHint)>,
) {
    let Ok((entity, mut color, mut hint)) = hint.single_mut() else {
        return;
    };

    if !lit.is_empty() {
        lit.clear();
        hint.dismissed = true;
    }

    let target = if hint.dismissed { 0.0 } else { HINT_ALPHA };
    let current = color.0.alpha();
    let step = (time.delta_secs() * 2.5).min(1.0);
    let alpha = current + (target - current) * step;
    color.0.set_alpha(alpha);

    if hint.dismissed && alpha <= 0.02 {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objective_alpha_holds_then_eases_to_idle() {
        assert!((objective_alpha(0.0) - OBJ_ACTIVE_ALPHA).abs() < 1e-6);
        assert!((objective_alpha(OBJ_HOLD_SECS) - OBJ_ACTIVE_ALPHA).abs() < 1e-6);

        let settled = objective_alpha(OBJ_HOLD_SECS + OBJ_FADE_SECS + 10.0);
        assert!((settled - OBJ_IDLE_ALPHA).abs() < 1e-6);

        // Monotonically non-increasing while fading, and never below idle.
        let early = objective_alpha(OBJ_HOLD_SECS + 0.4);
        let later = objective_alpha(OBJ_HOLD_SECS + 0.9);
        assert!(early >= later);
        assert!(later >= OBJ_IDLE_ALPHA);
    }
}

use bevy::prelude::*;

/// Spawns the single 2D camera used by every screen (menu, gameplay, HUD and
/// game-over) in the game.
///
/// The camera is created exactly once at [`Startup`] rather than inside
/// individual screens (which is what the original template did in
/// `menu.rs`). Spawning it per-screen only worked because the template's
/// state machine was a one-way `Loading -> Menu -> Playing` path; once the
/// game gained a `Menu <-> Playing <-> GameOver` loop (see
/// [`crate::game_over`]), spawning a fresh camera every time a screen is
/// entered would silently pile up extra cameras, each one issuing its own
/// draw call and warning at runtime. A single long-lived camera avoids that
/// class of bug entirely.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

/// Anti-aliasing is switched off ([`Msaa::Off`]) because this is a crisp,
/// pixel-art-adjacent 2D game where MSAA would only blur sprite edges while
/// costing extra GPU time for no visual benefit.
fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Msaa::Off));
}

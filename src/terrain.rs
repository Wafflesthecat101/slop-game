//! Shared terrain shape.
//!
//! The open world is defined by a single pure function, [`height`], that maps
//! any world `(x, z)` coordinate to a ground elevation `y`. Both the mesh
//! builder ([`crate::world`]) and the player's ground-follow logic
//! ([`crate::player`]) call this same function, so the visible terrain and the
//! surface the player walks on can never drift out of sync.
//!
//! The shape is a deterministic sum of sines (no noise crate, no per-frame
//! allocation) — cheap to evaluate thousands of times per frame and identical
//! on every platform.

/// Half-size of the square world, in meters. The terrain spans
/// `-HALF_SIZE..=HALF_SIZE` on both the X and Z axes.
pub const HALF_SIZE: f32 = 200.0;

/// Ground elevation (meters) at world position `(x, z)`.
pub fn height(x: f32, z: f32) -> f32 {
    // A few sine waves at different frequencies/directions sum into rolling
    // hills. The constants are arbitrary but fixed so the world is stable.
    let rolling = (x * 0.02).sin() * (z * 0.02).cos() * 12.0;
    let hills = (x * 0.045 + 1.3).sin() * 5.0 + (z * 0.05 - 0.7).cos() * 5.0;
    let ripple = ((x + z) * 0.11).sin() * 1.2;
    rolling + hills + ripple
}

/// Upward surface normal of the terrain at `(x, z)`, estimated from nearby
/// heights via finite differences. Used to orient scattered objects and to
/// know how steep the ground is.
pub fn normal(x: f32, z: f32) -> bevy::math::Vec3 {
    use bevy::math::Vec3;
    let e = 0.5;
    let dx = height(x + e, z) - height(x - e, z);
    let dz = height(x, z + e) - height(x, z - e);
    Vec3::new(-dx, 2.0 * e, -dz).normalize()
}

/// Biome tint (linear RGB multiplier, 0..1) for the ground at elevation `y`.
///
/// Elevation drives a simple three-band biome: sandy lowlands near sea level,
/// lush green midlands, and pale rocky highlands. Returned as an RGB factor
/// that multiplies the grass base texture, giving the single terrain mesh a
/// varied, less monotonous look for free (no extra draw calls). Bands are
/// blended with `smoothstep` so there are no hard seams.
pub fn biome_tint(y: f32) -> [f32; 3] {
    let sand = [0.82, 0.72, 0.48];
    let grass = [0.45, 0.62, 0.30];
    let rock = [0.62, 0.60, 0.58];

    // Low -> sand..grass, high -> grass..rock.
    let to_grass = smoothstep(-6.0, 2.0, y);
    let to_rock = smoothstep(8.0, 18.0, y);
    let low = mix3(sand, grass, to_grass);
    mix3(low, rock, to_rock)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_deterministic() {
        assert_eq!(height(3.0, -7.0), height(3.0, -7.0));
    }

    #[test]
    fn height_stays_within_expected_band() {
        // Sum of the amplitudes above is 12 + 5 + 5 + 1.2 = 23.2.
        let mut extreme = 0.0f32;
        let mut x = -HALF_SIZE;
        while x <= HALF_SIZE {
            let mut z = -HALF_SIZE;
            while z <= HALF_SIZE {
                extreme = extreme.max(height(x, z).abs());
                z += 4.0;
            }
            x += 4.0;
        }
        assert!(extreme <= 23.3, "height {extreme} exceeded amplitude band");
    }

    #[test]
    fn normal_points_up() {
        // On terrain of modest slope the surface normal must have a dominant
        // upward (+Y) component.
        assert!(normal(10.0, 10.0).y > 0.5);
    }

    #[test]
    fn biome_tint_bands_are_ordered_and_bounded() {
        // Low ground is sandy (red dominant), high ground trends grey/rock.
        let low = biome_tint(-20.0);
        let high = biome_tint(30.0);
        for c in low.iter().chain(high.iter()) {
            assert!((0.0..=1.0).contains(c));
        }
        // Lowlands are noticeably warmer (more red) than the rocky highlands.
        assert!(low[0] > high[0] - 0.01);
    }
}

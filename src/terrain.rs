//! Shared terrain shape.
//!
//! The open world is defined by a single pure function, [`height`], that maps
//! any world `(x, z)` coordinate to a ground elevation `y`. Both the mesh
//! builder ([`crate::world`]) and the player's ground-follow logic
//! ([`crate::player`]) call this same function, so the visible terrain and the
//! surface the player walks on can never drift out of sync.
//!
//! The shape is a deterministic **layered** function: a few low-frequency
//! sines give large-scale continental form, and several octaves of hash-based
//! value noise (fractional Brownian motion) add natural, non-repeating hills,
//! ridges and valleys on top. Everything is integer-hash + `f32` arithmetic
//! (no noise crate, no per-frame allocation, no platform-specific transcendental
//! in the noise itself), so it is cheap to evaluate thousands of times per
//! frame and identical on every platform.

/// Half-size of the square world, in meters. The terrain spans
/// `-HALF_SIZE..=HALF_SIZE` on both the X and Z axes.
pub const HALF_SIZE: f32 = 200.0;

/// Peak magnitude of the layered value-noise detail added on top of the
/// continental sines. See [`fbm`] for how the octaves sum.
const NOISE_AMP: f32 = 5.0;
/// World-space frequency of the first (largest) noise octave.
const NOISE_BASE_FREQ: f32 = 0.02;
/// Number of value-noise octaves summed by [`fbm`].
const NOISE_OCTAVES: u32 = 4;

/// Ground elevation (meters) at world position `(x, z)`.
pub fn height(x: f32, z: f32) -> f32 {
    continent(x, z) + fbm(x, z)
}

/// Large-scale, smooth continental form from a few low-frequency sines. This
/// is the gentle rolling backbone the noise detail rides on. Peak magnitude is
/// `10 + 4 + 4 = 18` m.
fn continent(x: f32, z: f32) -> f32 {
    let broad = (x * 0.012).sin() * (z * 0.012).cos() * 10.0;
    let ridges = (x * 0.03 + 1.3).sin() * 4.0 + (z * 0.028 - 0.7).cos() * 4.0;
    broad + ridges
}

/// Fractional Brownian motion: sum [`NOISE_OCTAVES`] octaves of value noise,
/// each at double the frequency and half the amplitude of the last (persistence
/// 0.5). The normalized octave sum lies in `[-1, 1]`; scaling by [`NOISE_AMP`]
/// bounds the whole noise term to `±NOISE_AMP`, keeping [`height`] within a
/// predictable band.
fn fbm(x: f32, z: f32) -> f32 {
    let mut freq = NOISE_BASE_FREQ;
    let mut amp = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..NOISE_OCTAVES {
        sum += amp * value_noise(x * freq, z * freq);
        norm += amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    // `sum / norm` renormalizes the octave sum back into `[-1, 1]`.
    (sum / norm) * NOISE_AMP
}

/// Smooth value noise sampled at `(x, z)`: hash the four surrounding integer
/// lattice points to values in `[-1, 1]` and bilinearly interpolate them with a
/// smoothstep fade. Deterministic and allocation-free.
fn value_noise(x: f32, z: f32) -> f32 {
    let xi = x.floor();
    let zi = z.floor();
    let fx = x - xi;
    let fz = z - zi;
    // Smoothstep fade removes the grid-aligned creases plain bilinear would show.
    let u = fx * fx * (3.0 - 2.0 * fx);
    let w = fz * fz * (3.0 - 2.0 * fz);

    let (ix, iz) = (xi as i32, zi as i32);
    let v00 = hash(ix, iz);
    let v10 = hash(ix + 1, iz);
    let v01 = hash(ix, iz + 1);
    let v11 = hash(ix + 1, iz + 1);

    let bottom = v00 + (v10 - v00) * u;
    let top = v01 + (v11 - v01) * u;
    bottom + (top - bottom) * w
}

/// Hash an integer lattice point to a pseudorandom value in `[-1, 1]`.
///
/// Pure integer mixing (wrapping arithmetic + xor-shifts) so it is fully
/// deterministic and identical on every platform; the final mask guarantees the
/// output range is exactly `[-1, 1]`, which is what lets [`fbm`]'s amplitude
/// bound hold.
fn hash(ix: i32, iz: i32) -> f32 {
    let mut h = (ix as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((iz as u32).wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    // Take 24 bits -> [0, 1] -> [-1, 1].
    let unit = (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32;
    unit * 2.0 - 1.0
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
        // Continental sines peak at 10 + 4 + 4 = 18 m; the fbm noise term is
        // bounded to +/- NOISE_AMP (5 m) by its normalization. So |height| can
        // never exceed 23 m; sample the map densely to confirm.
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
        assert!(extreme <= 23.0, "height {extreme} exceeded amplitude band");
    }

    #[test]
    fn height_has_noise_variation_between_octave_boundaries() {
        // The layered noise must add sub-continental detail: two points close
        // together (well inside one continental wavelength) should still differ,
        // which the old pure-sine terrain barely did at this scale.
        let a = height(12.3, -8.1);
        let b = height(13.7, -8.1);
        assert!(
            (a - b).abs() > 1e-4,
            "terrain looks flat/noiseless: {a} vs {b}"
        );
    }

    #[test]
    fn normal_is_unit_and_points_up_everywhere() {
        // By construction the finite-difference normal has a positive +Y term
        // before normalization, so it must stay unit-length and upward across
        // the whole (now noisier) map.
        let mut x = -HALF_SIZE;
        while x <= HALF_SIZE {
            let mut z = -HALF_SIZE;
            while z <= HALF_SIZE {
                let n = normal(x, z);
                assert!(
                    (n.length() - 1.0).abs() < 1e-3,
                    "normal not unit at ({x},{z})"
                );
                assert!(n.y > 0.0, "normal not upward at ({x},{z})");
                z += 8.0;
            }
            x += 8.0;
        }
        // On gently rolling ground the up component still dominates.
        assert!(normal(10.0, 10.0).y > 0.5);
    }

    #[test]
    fn enough_flat_hilltops_for_placement() {
        // Shrines and scenery are placed on cells that are high (`>= 6 m`) and
        // fairly flat (`normal.y >= 0.9`). The layered noise must not make the
        // world so jagged that placement starves — assert a healthy supply of
        // acceptable cells across the map so 12 well-separated shrines still fit.
        let reach = HALF_SIZE - 10.0;
        let mut candidates = 0;
        let mut x = -reach;
        while x <= reach {
            let mut z = -reach;
            while z <= reach {
                if height(x, z) >= 6.0 && normal(x, z).y >= 0.9 {
                    candidates += 1;
                }
                z += 3.0;
            }
            x += 3.0;
        }
        assert!(
            candidates > 500,
            "only {candidates} flat hilltop cells — placement may starve"
        );
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

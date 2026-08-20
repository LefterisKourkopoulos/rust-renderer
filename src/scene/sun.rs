//! Pure hour-of-day -> sun direction/color/intensity math, with no GPU dependency.
//!
//! `Light.direction` is the direction light *travels* (from the sun toward the ground), so at
//! noon, with the sun straight overhead, the direction is straight down (`[0, -1, 0]`).

/// Fixed compass heading for the sun's horizontal swing; a full azimuth sweep is a nice-to-have,
/// not required by the current control surface.
const AZIMUTH_DEG: f32 = 235.0;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SunState {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

/// A color/intensity keyframe at a given hour (0-24), interpolated linearly between neighbors.
struct Keyframe {
    hour: f32,
    color: [f32; 3],
    intensity: f32,
}

const KEYFRAMES: [Keyframe; 4] = [
    Keyframe { hour: 0.0, color: [0.05, 0.08, 0.15], intensity: 0.05 },
    Keyframe { hour: 6.0, color: [1.0, 0.55, 0.25], intensity: 0.6 },
    Keyframe { hour: 12.0, color: [1.0, 0.98, 0.92], intensity: 1.5 },
    Keyframe { hour: 18.0, color: [1.0, 0.45, 0.2], intensity: 0.6 },
];

/// Computes the sun's direction, color, and intensity for a given hour of day (0-24, wrapping).
pub fn sun_for_hour(hour: f32) -> SunState {
    SunState {
        direction: direction_for_hour(hour),
        color: color_for_hour(hour),
        intensity: intensity_for_hour(hour),
    }
}

fn direction_for_hour(hour: f32) -> [f32; 3] {
    let elevation = (std::f32::consts::PI * (hour - 6.0) / 12.0).sin() * 90.0_f32.to_radians();
    let azimuth = AZIMUTH_DEG.to_radians();

    // Unit vector from the scene toward the sun's position in the sky.
    let sun_up = [
        elevation.cos() * azimuth.cos(),
        elevation.sin(),
        elevation.cos() * azimuth.sin(),
    ];

    // The light travels the opposite way: from the sun down toward the ground.
    [-sun_up[0], -sun_up[1], -sun_up[2]]
}

fn color_for_hour(hour: f32) -> [f32; 3] {
    let (a, b, t) = neighbors(hour);
    lerp3(a.color, b.color, t)
}

fn intensity_for_hour(hour: f32) -> f32 {
    let (a, b, t) = neighbors(hour);
    a.intensity + (b.intensity - a.intensity) * t
}

/// Finds the two keyframes that bracket `hour` (wrapping past 24 back to the first one) and how
/// far between them `hour` falls, as a fraction in [0, 1].
fn neighbors(hour: f32) -> (&'static Keyframe, &'static Keyframe, f32) {
    let hour = hour.rem_euclid(24.0);

    for window in KEYFRAMES.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        if hour >= a.hour && hour <= b.hour {
            return (a, b, (hour - a.hour) / (b.hour - a.hour));
        }
    }

    // Between the last keyframe and midnight (wrapping around to the first).
    let a = KEYFRAMES.last().unwrap();
    let b = &KEYFRAMES[0];
    let span = 24.0 - a.hour + b.hour;
    let elapsed = if hour >= a.hour { hour - a.hour } else { hour + 24.0 - a.hour };
    (a, b, elapsed / span)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn magnitude(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    #[test]
    fn noon_points_the_sun_straight_overhead() {
        let sun = sun_for_hour(12.0);

        assert!((sun.direction[0]).abs() < EPSILON, "{:?}", sun.direction);
        assert!((sun.direction[1] - -1.0).abs() < EPSILON, "{:?}", sun.direction);
        assert!((sun.direction[2]).abs() < EPSILON, "{:?}", sun.direction);
    }

    #[test]
    fn sunrise_and_sunset_sit_at_the_horizon() {
        for hour in [6.0, 18.0] {
            let sun = sun_for_hour(hour);
            assert!(sun.direction[1].abs() < EPSILON, "hour {hour}: {:?}", sun.direction);
        }
    }

    #[test]
    fn midnight_is_dim_and_cool() {
        let sun = sun_for_hour(0.0);

        assert!(sun.intensity < 0.1, "{}", sun.intensity);
        assert!(sun.color[2] > sun.color[0], "midnight should read as cool/blue: {:?}", sun.color);
    }

    #[test]
    fn noon_matches_the_scene_configs_default_sun() {
        let sun = sun_for_hour(12.0);

        assert!((sun.intensity - 1.5).abs() < EPSILON);
        for (actual, expected) in sun.color.iter().zip([1.0, 0.98, 0.92]) {
            assert!((actual - expected).abs() < EPSILON, "{:?}", sun.color);
        }
    }

    #[test]
    fn sunrise_and_sunset_read_as_warm() {
        for hour in [6.0, 18.0] {
            let sun = sun_for_hour(hour);
            assert!(sun.color[0] > sun.color[2], "hour {hour} should read warm: {:?}", sun.color);
        }
    }

    #[test]
    fn the_cycle_wraps_smoothly_across_midnight() {
        let just_before = sun_for_hour(23.999);
        let just_after = sun_for_hour(0.001);

        assert!(
            (just_before.intensity - just_after.intensity).abs() < 0.01,
            "{} vs {}",
            just_before.intensity,
            just_after.intensity
        );
    }

    #[test]
    fn direction_is_always_a_unit_vector() {
        for hour in [0.0, 3.0, 6.0, 9.0, 12.0, 15.0, 18.0, 21.0, 23.9] {
            let sun = sun_for_hour(hour);
            assert!(
                (magnitude(sun.direction) - 1.0).abs() < EPSILON,
                "hour {hour}: {:?}",
                sun.direction
            );
        }
    }

    #[test]
    fn hours_outside_zero_to_twenty_four_wrap_like_a_clock() {
        let wrapped = sun_for_hour(24.0 + 6.0);
        let plain = sun_for_hour(6.0);

        assert!((wrapped.direction[1] - plain.direction[1]).abs() < EPSILON);
        assert!((wrapped.intensity - plain.intensity).abs() < EPSILON);
    }
}

const MIDNIGHT_AZIMUTH_DEG: f32 = 235.0;

const AZIMUTH_DEG_PER_HOUR: f32 = 360.0 / 24.0;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SunState {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

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

pub fn sun_for_hour(hour: f32, latitude_deg: f32) -> SunState {
    SunState {
        direction: direction_for_hour(hour, latitude_deg),
        color: color_for_hour(hour),
        intensity: intensity_for_hour(hour),
    }
}

fn direction_for_hour(hour: f32, latitude_deg: f32) -> [f32; 3] {
    let noon_elevation_deg = (90.0 - latitude_deg.abs()).clamp(0.0, 90.0);
    let elevation = (std::f32::consts::PI * (hour - 6.0) / 12.0).sin() * noon_elevation_deg.to_radians();
    let azimuth = (MIDNIGHT_AZIMUTH_DEG + hour * AZIMUTH_DEG_PER_HOUR).to_radians();

    let sun_up = [
        elevation.cos() * azimuth.cos(),
        elevation.sin(),
        elevation.cos() * azimuth.sin(),
    ];

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

fn neighbors(hour: f32) -> (&'static Keyframe, &'static Keyframe, f32) {
    let hour = hour.rem_euclid(24.0);

    for window in KEYFRAMES.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        if hour >= a.hour && hour <= b.hour {
            return (a, b, (hour - a.hour) / (b.hour - a.hour));
        }
    }

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
    const LONDON_LATITUDE_DEG: f32 = 51.5074;

    fn magnitude(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    #[test]
    fn at_the_equator_noon_points_the_sun_straight_overhead() {
        let sun = sun_for_hour(12.0, 0.0);

        assert!((sun.direction[0]).abs() < EPSILON, "{:?}", sun.direction);
        assert!((sun.direction[1] - -1.0).abs() < EPSILON, "{:?}", sun.direction);
        assert!((sun.direction[2]).abs() < EPSILON, "{:?}", sun.direction);
    }

    #[test]
    fn londons_latitude_caps_noon_elevation_well_short_of_the_zenith() {
        let sun = sun_for_hour(12.0, LONDON_LATITUDE_DEG);

        assert!(
            sun.direction[1] > -0.85,
            "London's noon sun should not approach the zenith: {:?}",
            sun.direction
        );
    }

    #[test]
    fn a_higher_latitude_gives_a_lower_noon_elevation() {
        let low_latitude = sun_for_hour(12.0, 10.0);
        let high_latitude = sun_for_hour(12.0, 60.0);

        assert!(
            low_latitude.direction[1] < high_latitude.direction[1],
            "10 degrees {:?} should reach a higher elevation than 60 degrees {:?}",
            low_latitude.direction,
            high_latitude.direction
        );
    }

    #[test]
    fn sunrise_and_sunset_sit_at_the_horizon() {
        for hour in [6.0, 18.0] {
            let sun = sun_for_hour(hour, LONDON_LATITUDE_DEG);
            assert!(sun.direction[1].abs() < EPSILON, "hour {hour}: {:?}", sun.direction);
        }
    }

    #[test]
    fn midnight_is_dim_and_cool() {
        let sun = sun_for_hour(0.0, LONDON_LATITUDE_DEG);

        assert!(sun.intensity < 0.1, "{}", sun.intensity);
        assert!(sun.color[2] > sun.color[0], "midnight should read as cool/blue: {:?}", sun.color);
    }

    #[test]
    fn noon_matches_the_scene_configs_default_sun() {
        let sun = sun_for_hour(12.0, LONDON_LATITUDE_DEG);

        assert!((sun.intensity - 1.5).abs() < EPSILON);
        for (actual, expected) in sun.color.iter().zip([1.0, 0.98, 0.92]) {
            assert!((actual - expected).abs() < EPSILON, "{:?}", sun.color);
        }
    }

    #[test]
    fn sunrise_and_sunset_read_as_warm() {
        for hour in [6.0, 18.0] {
            let sun = sun_for_hour(hour, LONDON_LATITUDE_DEG);
            assert!(sun.color[0] > sun.color[2], "hour {hour} should read warm: {:?}", sun.color);
        }
    }

    #[test]
    fn the_cycle_wraps_smoothly_across_midnight() {
        let just_before = sun_for_hour(23.999, LONDON_LATITUDE_DEG);
        let just_after = sun_for_hour(0.001, LONDON_LATITUDE_DEG);

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
            let sun = sun_for_hour(hour, LONDON_LATITUDE_DEG);
            assert!(
                (magnitude(sun.direction) - 1.0).abs() < EPSILON,
                "hour {hour}: {:?}",
                sun.direction
            );
        }
    }

    #[test]
    fn the_suns_compass_heading_sweeps_across_the_sky_rather_than_holding_still() {
        let heading = |d: [f32; 3]| d[0].atan2(d[2]).to_degrees();

        let sunrise = heading(sun_for_hour(6.0, LONDON_LATITUDE_DEG).direction);
        let sunset = heading(sun_for_hour(18.0, LONDON_LATITUDE_DEG).direction);

        assert!(
            (sunrise - sunset).abs() > 90.0,
            "sunrise heading {sunrise} and sunset heading {sunset} should differ, \
             like a real sun rising and setting on opposite sides of the sky"
        );
    }

    #[test]
    fn the_azimuth_sweep_completes_one_full_turn_per_day() {
        let hour_three = sun_for_hour(3.0, LONDON_LATITUDE_DEG).direction;
        let one_day_later = sun_for_hour(27.0, LONDON_LATITUDE_DEG).direction;

        for (a, b) in hour_three.iter().zip(one_day_later.iter()) {
            assert!((a - b).abs() < EPSILON, "{hour_three:?} vs {one_day_later:?}");
        }
    }

    #[test]
    fn hours_outside_zero_to_twenty_four_wrap_like_a_clock() {
        let wrapped = sun_for_hour(24.0 + 6.0, LONDON_LATITUDE_DEG);
        let plain = sun_for_hour(6.0, LONDON_LATITUDE_DEG);

        assert!((wrapped.direction[1] - plain.direction[1]).abs() < EPSILON);
        assert!((wrapped.intensity - plain.intensity).abs() < EPSILON);
    }
}

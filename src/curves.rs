use crate::ControlPoint;

/// Linear interpolation between (start_time, start_value) and (end_time, end_value).
pub fn create_curve(start_time: f64, end_time: f64, start_value: f64, end_value: f64, steps: usize) -> Vec<ControlPoint> {
    let steps = steps.max(1);
    let time_diff = end_time - start_time;
    let value_diff = end_value - start_value;
    let time_step = time_diff / steps as f64;
    let value_step = value_diff / steps as f64;

    (0..steps)
        .map(|i| ControlPoint {
            time: start_time + time_step * (i as f64 + 1.0),
            parameter_value: start_value + value_step * (i as f64 + 1.0),
        })
        .collect()
}

pub fn linear_interpolation(start: ControlPoint, end: ControlPoint, steps: usize) -> Vec<ControlPoint> {
    create_curve(start.time, end.time, start.parameter_value, end.parameter_value, steps)
}

/// Smoothstep-based ease-in-out curve.
pub fn ease_in_out(start: ControlPoint, end: ControlPoint, steps: usize) -> Vec<ControlPoint> {
    let steps = steps.max(1);
    let time_diff = end.time - start.time;
    let value_diff = end.parameter_value - start.parameter_value;

    (0..steps)
        .map(|i| {
            let t = (i as f64 + 1.0) / steps as f64;
            let smooth_t = t * t * (3.0 - 2.0 * t);
            ControlPoint {
                time: start.time + time_diff * t,
                parameter_value: start.parameter_value + value_diff * smooth_t,
            }
        })
        .collect()
}

pub fn exponential(start: ControlPoint, end: ControlPoint, steps: usize, exponent: f64) -> Vec<ControlPoint> {
    let steps = steps.max(1);
    let time_diff = end.time - start.time;
    let value_diff = end.parameter_value - start.parameter_value;

    (0..steps)
        .map(|i| {
            let t = (i as f64 + 1.0) / steps as f64;
            let exp_t = t.powf(exponent);
            ControlPoint {
                time: start.time + time_diff * t,
                parameter_value: start.parameter_value + value_diff * exp_t,
            }
        })
        .collect()
}

/// Converts a frequency in Hz to a sharpness value in [0, 1] using a log
/// mapping between 80 Hz and 230 Hz. With `normalize = true`, out-of-range
/// frequencies are clamped instead of rejected.
pub fn freq_to_sharpness(freq: f64, normalize: bool) -> Result<f64, String> {
    let mut freq = freq;
    if normalize {
        freq = freq.clamp(80.0, 230.0);
    }

    if !(80.0..=230.0).contains(&freq) {
        return Err(format!(
            "incorrect frequency: frequency must be between 80 and 230, but it is {:.2}",
            freq
        ));
    }

    let r = (freq.ln() - 80f64.ln()) / (230f64.ln() - 80f64.ln());

    if !(0.0..=1.0).contains(&r) {
        return Err("the calculated normalized frequency is out of range: result must be between 0 and 1".into());
    }

    Ok(r)
}

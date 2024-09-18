// a simple example to create an AHAP file for a bike sound
// a harley davidson bike sound is created with haptic feedback xD


#![allow(dead_code)]
mod ahap;

use ahap::{AHAP, CurveParamID};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create a new AHAP file
    let mut ahap = AHAP::new("bike sound made in rust", "Deniz Sincar");
    let mut time = 0.0;  // keep track of time
    // first continuous event and the subsequent 7 transient events are indicating the starter of the engine.
    let dur = 0.4;  // duration of the first event
    ahap.add_haptic_continuous_event(time, dur, 0.5, 0.4);
    ahap.add_parameter_curve(CurveParamID::HapticSharpnessControl, time, AHAP::create_curve(0.0, 0.4, 0.4, 0.75, 10));
    time += 0.45;
    for _ in 0..7 {
        ahap.add_haptic_transient_event(time, 1.0, 0.3);
        time += 0.05;
    }
    // the engine is started, now we add a continuous event to indicate the engine sound
    ahap.add_haptic_continuous_event(time, 15.0, 0.75, 0.0);
    for i in 0..300 {
        // these are the pukpukpuk sounds of the engine. They are played along with the continuous event.
        ahap.add_haptic_transient_event(time + i as f32 * 0.05, 1.0, 1.0);
    }
    // what we need to do s=while the engine is starting to prevent the engine from stopping? Right, we need to give gas a little bit.
    ahap.add_parameter_curve(CurveParamID::HapticSharpnessControl, time, AHAP::create_curve(0.0, 0.4, 0.0, 0.75, 10));
    time += 0.4;
    // and we release the gas
    ahap.add_parameter_curve(CurveParamID::HapticSharpnessControl, time, AHAP::create_curve(0.0, 0.8, 0.75, 0.2, 10));
    time += 0.8;
    // now we are moving the bike, we need to give gas again. First gear.
    ahap.add_parameter_curve(CurveParamID::HapticSharpnessControl, time, AHAP::create_curve(0.0, 3.0, 0.0, 0.5, 10));
    // gear up
    ahap.add_parameter_curve(CurveParamID::HapticSharpnessControl, time + 3.0, AHAP::create_curve(0.0, 3.0, 0.2, 0.65, 10));
    // one more gear up
    ahap.add_parameter_curve(CurveParamID::HapticSharpnessControl, time + 6.0, AHAP::create_curve(0.0, 4.0, 0.4, 1.0, 10));
    // and we are going by inertia and decreasing the gas
    ahap.add_parameter_curve(CurveParamID::HapticSharpnessControl, time + 10.0, AHAP::create_curve(0.0, 2.0, 1.0, 0.0, 10));
    // and we done!

    ahap.export_pretty("bike.ahap")?;

    Ok(())
}

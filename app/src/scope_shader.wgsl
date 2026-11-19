// The scope shader library: the look every scope (the waveforms and the vectorscope) shares. It is put in front
// of each scope's draw shader (`scope_shader::with_library`), so a change here changes all of them.

// Neutral greys only: nothing on screen may tint the eye.
const PLOT: vec3<f32> = vec3<f32>(0.035, 0.035, 0.035);
const GRID: vec3<f32> = vec3<f32>(0.14, 0.14, 0.14);
const TRACE: vec3<f32> = vec3<f32>(0.86, 0.86, 0.86);

// Phosphor response: 1 - exp(-gain * density), where `density` is relative to what the Panel and frame size lead
// the scope to expect (`expected`), and never below `floor_level` while even one pixel is there. `gain` is the
// trace brightness setting.
fn phosphor(gain: f32, v: f32, expected: f32, floor_level: f32) -> f32 {
    if (v <= 0.0) {
        return 0.0;
    }
    return clamp(max(floor_level, 1.0 - exp(-gain * v / expected)), 0.0, 1.0);
}

// Soft spread: how far (in device pixels) a trace is spread over its neighbours when the soft spread is on.
fn spread_step(device_scale: f32) -> f32 {
    return max(1.0, device_scale);
}

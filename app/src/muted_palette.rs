//! The heat palette of the Picture Views that map a measurement to colour (the saturation heatmap, #64, #100): the
//! picture stays as its Luma in grey, and the measurement is laid over it in a colour whose strength rises with
//! the measurement, so neutral areas show the grey picture and strongly saturated areas a strong colour, cool (blue)
//! to hot (red). The ramp is chosen for contrast, not for even brightness (ADR 0023).

/// The stops, evenly spaced from cool (0) to hot (1), as encoded RGB.
pub const STOPS: [[f32; 3]; 6] = [
    [0.20, 0.45, 0.95], // blue
    [0.10, 0.78, 0.88], // cyan
    [0.25, 0.85, 0.30], // green
    [0.98, 0.88, 0.15], // yellow
    [1.00, 0.52, 0.08], // orange
    [0.95, 0.12, 0.12], // red
];

/// Where the colour reaches full strength, as a share of the scale: below it the grey picture shows through.
pub const FULL_STRENGTH_AT: f32 = 0.3;

/// The ramp's colour at `t` (0 cool to 1 hot; outside is clamped), blending linearly between stops.
pub fn heat(t: f32) -> [f32; 3] {
    let x = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (x.floor() as usize).min(STOPS.len() - 2);
    let f = x - i as f32;
    [0, 1, 2].map(|c| STOPS[i][c] + (STOPS[i + 1][c] - STOPS[i][c]) * f)
}

/// How much of the colour is laid over the grey at `t`: none at 0, all of it from [`FULL_STRENGTH_AT`], eased in (smoothstep) so a hair off neutral stays grey.
pub fn strength(t: f32) -> f32 {
    {
        let s = (t / FULL_STRENGTH_AT).clamp(0.0, 1.0);
        s * s * (3.0 - 2.0 * s)
    }
}

/// What is drawn for a pixel whose Luma is `grey` and whose measurement is at `t` on the scale.
pub fn overlay(grey: f32, t: f32) -> [f32; 3] {
    let (colour, s) = (heat(t), strength(t));
    colour.map(|c| grey + (c - grey) * s)
}

/// WGSL for `fn overlay(grey: f32, t: f32) -> vec3<f32>`, from [`STOPS`] so there is one table.
pub fn wgsl() -> String {
    let n = STOPS.len();
    let mut code = String::from("fn heat(t: f32) -> vec3<f32> {\n");
    code += &format!(
        "    let x = clamp(t, 0.0, 1.0) * {:?};\n    let i = min(u32(floor(x)), {}u);\n    let f = x - f32(i);\n",
        (n - 1) as f32,
        n - 2
    );
    code += &format!("    var stops = array<vec3<f32>, {n}>(\n");
    for s in STOPS {
        code += &format!("        vec3<f32>({:?}, {:?}, {:?}),\n", s[0], s[1], s[2]);
    }
    code += "    );\n    return mix(stops[i], stops[i + 1u], f);\n}\n";
    code += &format!(
        "fn overlay(grey: f32, t: f32) -> vec3<f32> {{\n    return mix(vec3<f32>(grey), heat(t), smoothstep(0.0, {:?}, t));\n}}\n",
        FULL_STRENGTH_AT
    );
    code
}

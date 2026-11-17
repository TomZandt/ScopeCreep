//! The muted, even-brightness palette the Picture Views that map a measurement to colour share (the saturation
//! heatmap, #64, and the Skin Tone Panel, #63): cool at 0, warm at 1. Every stop has the same Rec.709 luma
//! (about 0.56), so no part of the scale is brighter than another and it can be watched through a long session.

/// The stops, evenly spaced from cool (0) to warm (1), as encoded RGB.
pub const STOPS: [[f32; 3]; 7] = [
    [0.409, 0.579, 0.817], // cool blue
    [0.336, 0.615, 0.671],
    [0.317, 0.633, 0.554],
    [0.326, 0.653, 0.326],
    [0.543, 0.592, 0.296],
    [0.687, 0.544, 0.344],
    [0.856, 0.485, 0.428], // warm red
];

/// The colour at `t` (0 cool to 1 warm; outside is clamped), blending linearly between stops.
pub fn muted(t: f32) -> [f32; 3] {
    let x = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (x.floor() as usize).min(STOPS.len() - 2);
    let f = x - i as f32;
    [0, 1, 2].map(|c| STOPS[i][c] + (STOPS[i + 1][c] - STOPS[i][c]) * f)
}

/// WGSL for `fn muted(t: f32) -> vec3<f32>`, from [`STOPS`] so there is one table.
pub fn wgsl() -> String {
    let n = STOPS.len();
    let mut code = String::from("fn muted(t: f32) -> vec3<f32> {\n");
    code += &format!("    let x = clamp(t, 0.0, 1.0) * {:?};\n    let i = min(u32(floor(x)), {}u);\n    let f = x - f32(i);\n", (n - 1) as f32, n - 2);
    code += &format!("    var stops = array<vec3<f32>, {n}>(\n");
    for s in STOPS {
        code += &format!("        vec3<f32>({:?}, {:?}, {:?}),\n", s[0], s[1], s[2]);
    }
    code += "    );\n    return mix(stops[i], stops[i + 1u], f);\n}\n";
    code
}

//! The style check (spec #113, ADR 0026): chrome code draws only through the style tokens, so it holds no raw
//! colour or size literal. Measurement colours (false colour bands, heatmap palette, skin tints, trace colours)
//! live in the display modules, which are not chrome and are exempt by rule, so a band is never "corrected" to a
//! theme colour.

/// What is wrong in `source`, one line each: a raw colour (`Color::rgb(138, ..)`, `wgpu::Color { r: ..}`, an
/// array of three or four number literals) or a raw size (`const X: f32 = 12.0`, `size: 12.0`,
/// `Metrics::new(12.0, ..)`). Comments and everything from `#[cfg(test)]` on are ignored.
fn raw_literals(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (n, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        let code = line.split("//").next().unwrap_or("");
        let t = code.trim();
        let digit_after = |pat: &str| {
            code.match_indices(pat).any(|(i, _)| {
                code[i + pat.len()..]
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit())
            })
        };
        let mut bad = digit_after("Color::rgb(")
            || digit_after("Color::rgba(")
            || code.contains("wgpu::Color {");
        // A text size is a float (`size: 12.5`); a GPU buffer's byte `size: 16` is not a style size.
        let float_after = |pat: &str| {
            code.match_indices(pat).any(|(i, _)| {
                let rest = code[i + pat.len()..].trim_start();
                let digits = rest.trim_start_matches(|c: char| c.is_ascii_digit());
                digits.len() < rest.len() && digits.starts_with('.')
            })
        };
        bad |= float_after("size:") || float_after("Metrics::new(");
        if t.contains("const ") && t.contains("= ") {
            let rhs = t.split("= ").nth(1).unwrap_or("").trim_start();
            bad |= t.contains(": f32 =") && rhs.starts_with(|c: char| c.is_ascii_digit());
            bad |= t.contains(": [f32")
                && rhs.starts_with('[')
                && rhs[1..]
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit());
        }
        for (i, _) in code.match_indices('[') {
            if let Some(end) = code[i..].find(']') {
                let parts: Vec<&str> = code[i + 1..i + end].split(',').map(str::trim).collect();
                if (parts.len() == 3 || parts.len() == 4)
                    && parts
                        .iter()
                        .all(|p| p.contains('.') && p.parse::<f32>().is_ok())
                {
                    bad = true;
                }
            }
        }
        if bad {
            found.push(format!("line {}: {}", n + 1, t));
        }
    }
    found
}

/// The chrome: everything that draws or lays out the Window's frame rather than a measurement.
const CHROME: [&str; 6] = [
    "src/components.rs",
    "src/main.rs",
    "src/settings.rs",
    "src/panel_grid.rs",
    "src/layout.rs",
    "src/flat_rects.rs",
];

#[test]
fn chrome_code_holds_no_raw_colour_or_size() {
    for file in CHROME {
        let path = format!("{}/{file}", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let found = raw_literals(&source);
        assert!(
            found.is_empty(),
            "{file} has raw colour or size literals, use the tokens in style.rs:\n{}",
            found.join("\n")
        );
    }
}

#[test]
fn the_check_fails_on_a_planted_raw_colour_or_size() {
    for planted in [
        "const LABEL_COLOUR: Color = Color::rgb(138, 138, 138);",
        "rects.push((rect, [0.11, 0.11, 0.11, 1.0]));",
        "let c = wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 };",
        "const NOTICE_SIZE: f32 = 15.0;",
        "TextStyle { size: 12.5, line: 1.4 }",
        "Metrics::new(12.0, 16.0)",
        "const MIN: [f32; 2] = [120.0, 110.0];",
    ] {
        assert_eq!(
            raw_literals(planted).len(),
            1,
            "should be caught: {planted}"
        );
    }
}

#[test]
fn the_check_passes_tokens_comments_and_ordinary_arithmetic() {
    for fine in [
        "const LABEL: TextStyle = TextStyle { size: style::TYPE_LABEL, line: style::TYPE_LABEL_LINE };",
        "rects.push((rect, style::rgba(style::SURFACE_MENU)));",
        "let line = style::TYPE_LABEL * scale * 2.0;",
        "// a grey of Color::rgb(138, 138, 138) in a comment",
        "let [left, top, _, height] = cell;",
        "let x = [a, b, c, d];",
    ] {
        assert!(raw_literals(fine).is_empty(), "should pass: {fine}");
    }
}

#[test]
fn a_measurement_colour_in_a_display_is_exempt() {
    // Displays are not in CHROME: the heatmap's palette is data and stays where it is.
    let display = std::fs::read_to_string(format!(
        "{}/src/muted_palette.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(
        !raw_literals(&display).is_empty(),
        "the palette holds raw colours, as measurement data does"
    );
    assert!(!CHROME.contains(&"src/muted_palette.rs"));
}

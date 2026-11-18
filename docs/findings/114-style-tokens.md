# 114: Style tokens by role and the chrome check

What the style tokens ticket (#114) verified, headlessly (`app/tests/style.rs`, `app/tests/style_check.rs`).

- The tokens are neutral (equal red, green, blue), text steps down primary, secondary, muted, surfaces step up window, menu, hover, border, and no chrome reaches white. The type scale is three sizes.
- The tokens keep the values the Window already used (menu border 0.30, fill 0.11, hover 0.20, on-mark 0.86, window 0.071, text 176/138/112, sizes 15/12.5/11, margin 12, menu row 24, status row 20): **no visible change**, and no near-identical values needed unifying.
- The check passes on the chrome files and was seen failing on planted raw colours (`Color::rgb`, rgba array, `wgpu::Color`) and sizes (`const`, `size:`, `Metrics::new`, a const array). A GPU buffer's byte `size: 16` is not flagged (only float sizes are).
- The heatmap palette, a measurement colour, is outside the chrome list and still holds raw colours: the exemption works.
- The full workspace tests share the GPU; when several agents run them at once, GPU tests can fail with `Device(OutOfMemory)`. This is not a defect of the change.

Not verified (a person at the app): that the Window looks the same on a real monitor. Folded into #105.

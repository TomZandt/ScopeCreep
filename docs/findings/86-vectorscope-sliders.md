# #86: trace brightness and soft spread sliders on the vectorscope

What ticket #86 showed on 2026-11-19, at the display seam (`vectorscope_seam.rs`), headless.

- Brightness is 20 to 400 % in steps of 5, default 100 %. The vectorscope's own gain scale is wider than the waveform's (old Brighter was 4 times, Dimmer a quarter), so the range reaches 400 % and the old list values are 400 and 25; saved `normal`, `bright` and `dim` draw the same pixels as 100, 400 and 25.
- Soft spread is 0 to 3 px in steps of 0.1, default 1.0 px (since changed to steps of 0.25, as on the waveforms, in the same milestone). The spread step is the old step (the larger of 1 and the device scale) times the amount, so 1.0 draws exactly as On, and 0 skips the spread and draws exactly as Off; saved `on` and `off` map to 1 and 0.
- Neither value changes the label; the ring, target boxes and skin-tone line are drawn as before.
- Not verified: how it looks and drags in the real Window and on HiDPI (#91).

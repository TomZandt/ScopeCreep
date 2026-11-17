# 51: Panel settings menu

What the right-click settings ticket (#51) verified, headlessly (no Resolve involved: the Feed seam and the pure seam).

- A display lists its settings as choices (the first is the default); a Panel with no settings (the clean picture) has no menu. Settings are kept per Panel name, round-trip through the saved text, and rubbish lines or a choice that is no longer offered are ignored in favour of the default (`app/tests/settings.rs`).
- The menu lists each setting with its choices and ticks the current one; a click lands on the choice under it and never on a heading, at scales 1 and 2; the menu stays inside the Window.
- Luma waveform, luma weights, with a P3-D65 project and a pure green frame (`app/tests/display_seam.rs`): "from primaries" puts every pixel at code 670 (P3-D65's own green weight 0.6917); "match Resolve" puts it at code 690 or 691 (Rec.709's 0.7152, as Resolve's own waveform does for P3-D65, ADR 0004). The label changes with it ("from primaries" / "to match Resolve", with the weights).
- A choice saved in one session applies in the next; a saved value from another version falls back to the default.
- The app starts and runs with the menu code without error.

Not verified (a person at the app and Resolve): the menu's look and behaviour with a real mouse, HiDPI, and the weights against Resolve's own waveform: https://github.com/TomZandt/scopecreep/issues/69

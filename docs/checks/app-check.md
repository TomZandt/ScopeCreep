# The standard app check

What a person at the app checks for any Panel or Window change. A queue checkbox says "standard app check, plus <the specifics>"; it does not repeat this list.

1. **Real monitor:** a real display at 1x, and at 1.5x and 2x HiDPI. Text, dashes and hairlines stay legible.
2. **Smallest Window:** the Window at its minimum size. Nothing overlaps or clips.
3. **Real mouse and trackpad:** every control the change adds, by hand.
4. **UHD 50 cost:** `harness/experiments/uhd50_bench` with the Sender off, on, and on with the app (ADR 0003). Resolve drops no frames.
5. **Record:** what was seen goes in `docs/findings/<ticket>-*.md`. Anything wrong becomes its own issue.

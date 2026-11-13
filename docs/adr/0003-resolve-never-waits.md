# Resolve never waits for scopecreep

Turning the Sender on must not change Resolve's playback frame rate. This constraint is invisible in the code but shapes all of it. The Sender's render call starts a copy and returns at once; it never blocks on the app, a lock or a full buffer. If the app falls behind, it drops Scope frames and shows that it did; it never slows Resolve. When no app is listening, the Sender does no work at all. Every change to the Sender or the transport is benchmarked with the Sender on and off at the project's resolution and frame rate.

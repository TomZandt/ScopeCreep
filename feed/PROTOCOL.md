# The Feed protocol, version 1.0

The Feed is how the **Sender** (inside Resolve) hands frames to the scopecreep app on the same machine. It is one shared-memory file holding a small header and a ring of frame slots. This document is the specification; `include/scopecreep_feed.h` (C, used by the Sender) and the `scopecreep-feed` Rust crate (used by the app) are two implementations of it, tested against each other (`tests/c_interop.rs`).

Think of it as a row of pigeonholes in a corridor between two rooms. The Sender drops a frame in a pigeonhole and walks on without looking up. The app empties pigeonholes when it gets to them. If every pigeonhole is full, the Sender replaces the oldest uncollected frame and makes a tally mark. Nobody ever waits in the corridor.

## Rules that never change

1. **Nobody waits.** No locks, no blocking calls. Every shared field that both sides touch is a lock-free atomic.
2. **One Sender process writes, one app reads.**
3. **A Feed file's layout never changes.** To change the slot count or slot size the Sender makes a new file, renames it over the old name, then marks the old file retired. A reader keeps a valid mapping of the old file until it re-opens.
4. **No reader, no work.** The Sender copies frames only while the reader's heartbeat is fresh.
5. All integers are little-endian. All offsets are fixed. Reserved bytes are written as zero and ignored when read.

## The file

Default path `/dev/shm/scopecreep-feed` (POSIX shared memory). `SCOPECREEP_FEED=<path>` overrides it for both sides (tests use this). Mode 0600.

```
[ Feed header : 4096 bytes ][ slot 0 ][ slot 1 ] ... [ slot N-1 ]
slot = [ slot header : 4096 bytes ][ pixel area : slot_capacity bytes ]
```

A Feed with `slot_count = 0` is valid: it means "a Sender is loaded but has not been asked for frames yet". The Sender publishes that file when it loads, and replaces it with a file that has slots the first time a reader is attached and a frame arrives (and again if a larger frame arrives later).

## Feed header (4096 bytes)

| Offset | Size | Field | Written by | Meaning |
|---|---|---|---|---|
| 0 | 8 | `magic` | Sender | bytes `SCFEED\0\0` |
| 8 | 4 | `version_major` | Sender | 1 |
| 12 | 4 | `version_minor` | Sender | 0 |
| 16 | 4 | `header_bytes` | Sender | 4096 |
| 20 | 4 | `slot_header_bytes` | Sender | 4096 |
| 24 | 4 | `slot_count` | Sender | number of slots, 0 = no ring yet |
| 28 | 4 | `retired` (atomic) | Sender | 1 = replaced by a newer Feed file; re-open the path |
| 32 | 8 | `slot_capacity` | Sender | bytes in each slot's pixel area (multiple of 4096) |
| 40 | 8 | `feed_id` | Sender | non-zero, different for every Feed file |
| 48 | 8 | `sender_pid` | Sender | process id of the Sender's host |
| 56 | 8 | reserved | | |
| 64 | 64 | `sender_name` | Sender | UTF-8, NUL padded: `scopecreep Sender` |
| 128 | 32 | `sender_version` | Sender | UTF-8, NUL padded |
| 160 | 64 | `host_name` | Sender | UTF-8, NUL padded: host name and version as the host reports them |
| 224 | 32 | reserved | | |
| 256 | 8 | `reader_heartbeat_ns` (atomic) | reader | monotonic time of the reader's last heartbeat, 0 = no reader |
| 264 | 8 | `reader_pid` (atomic) | reader | process id of the attached reader |
| 272 | 48 | reserved | | |
| 320 | 8 | `next_sequence` (atomic) | Sender | sequence numbers handed out so far |
| 328 | 8 | `frames_published` (atomic) | Sender | frames whose copy completed |
| 336 | 8 | `drops_overwritten` (atomic) | Sender | unread frames replaced because the ring was full |
| 344 | 8 | `drops_busy` (atomic) | Sender | frames skipped because nothing could be taken without waiting |
| 352 | 8 | `renders_total` (atomic) | Sender | every render call the Sender saw |
| 360 | 8 | `renders_thumbnail` (atomic) | Sender | thumbnail renders, never fed |
| 368 | 8 | `renders_idle` (atomic) | Sender | renders passed through with no reader attached (no copy) |
| 376 | 8 | `renders_unsupported` (atomic) | Sender | renders the Sender could not feed (not on the GPU path, or not 32-bit float RGBA) |
| 384 | 8 | `render_call_ns_total` (atomic) | Sender | time spent inside the Sender's render call, summed |
| 392 | 8 | `last_render_ns` (atomic) | Sender | monotonic time of the last render call |
| 400 | 112 | reserved | | |
| 512 | 256 | `sender_error` | Sender | UTF-8, NUL padded: the Sender's last error, empty = none |
| 768 | 3328 | reserved | | |

**Clock.** Every `_ns` time is the system-wide monotonic clock in nanoseconds (`CLOCK_MONOTONIC` on Linux), so both processes can compare times.

## Slot header (4096 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0 | 4 | `state` (atomic) | 0 FREE, 1 WRITING, 2 READY, 3 READING |
| 4 | 4 | `pixel_format` | 1 = RGBA, four IEEE 32-bit floats per pixel in R, G, B, A order |
| 8 | 8 | `sequence` (atomic) | 1, 2, 3 ... in the order the Sender took frames; gaps are drops |
| 16 | 4 | `width` | pixels |
| 20 | 4 | `height` | pixels |
| 24 | 4 | `row_bytes` | bytes from one row to the next (at least `width * 16`) |
| 28 | 4 | `row_order` | 0 = bottom row first (as OFX hands frames over), 1 = top row first |
| 32 | 8 | `data_bytes` | `row_bytes * height`, never more than `slot_capacity` |
| 40 | 8 | `frame_time` (f64) | source frame number on the timeline, as the host gave it |
| 48 | 8 | `frame_rate` (f64) | frames per second of the source timeline, 0 = unknown |
| 56 | 8 | `render_ns` | monotonic time the Sender's render call began |
| 64 | 8 | `publish_ns` | monotonic time the copy completed |
| 72 | 4 | `sender_instance` | which Sender instance made this frame: 1, 2, 3 ... in order of creation within the Sender process |
| 76 | 4 | `flags` | reserved, 0 |
| 80 | 944 | reserved | |
| 1024 | 4 | `colour_setup_provenance` | 0 = none, 1 = project, 2 = manual |
| 1028 | 4 | `colour_setup_bytes` | bytes used in `colour_setup`, 0 in version 1.0 |
| 1032 | 3064 | `colour_setup` | the Colour Setup known to the Sender. Format to be defined by a later minor version (ticket #16) |

Pixel values are the Sender's input exactly as Resolve rendered it: unclamped, in the Timeline Colour Space.

### Sender identity

Resolve tells an OFX plugin no project name, timeline name or page (Stage 0). So the identity of a frame's Sender is what is really available: `sender_pid` and `host_name` (header) plus `sender_instance` and `frame_rate` (slot). Project and timeline names must come from the scripting side (ticket #16) if they are wanted.

## Slot protocol

```
Sender:  FREE ──CAS──► WRITING ──(copy done)──► READY
         READY ─CAS──► WRITING   (oldest unread frame replaced: drops_overwritten += 1)
reader:  READY ─CAS──► READING ──(done)──► FREE
```

**Sender, per frame:**
1. Take any FREE slot (compare-and-swap to WRITING).
2. If none, take the READY slot with the lowest `sequence` and add 1 to `drops_overwritten`.
3. If none (every slot is WRITING or READING), skip the frame and add 1 to `drops_busy`.
4. Add 1 to `next_sequence`; the new value is this frame's `sequence`. Fill in the slot header, start the copy.
5. When the copy has completed, set `publish_ns`, add 1 to `frames_published`, and store READY.

**Reader:**
1. Pick the READY slot with the lowest `sequence`, compare-and-swap to READING. If the swap fails, look again.
2. Use the slot header and pixel area in place. The Sender never touches a READING slot.
3. Store FREE.

Frames are therefore delivered in order, and a slow reader loses the oldest frames first. Total frames lost so far is `drops_overwritten + drops_busy`.

## Reader presence

- The reader stores the monotonic time into `reader_heartbeat_ns` at least every 250 ms, and its process id into `reader_pid`.
- The reader is **present** while `now - reader_heartbeat_ns < 1 s`. Otherwise the Sender copies nothing (it only passes the picture through and adds 1 to `renders_idle`).
- On a clean exit the reader stores 0 into `reader_heartbeat_ns`. If the reader is killed, the heartbeat goes stale within a second.
- A reader that finds a fresh heartbeat with a different `reader_pid` must not attach (another app is using the Feed).
- When a reader attaches, it stores FREE into every slot that is READING: those belong to a reader that died.

## Opening a Feed (reader)

1. Open the path. Missing file: there is no Sender. This is a normal state, not an error.
2. Map the first 4096 bytes. If `magic` does not match, the file is not a Feed.
3. If `version_major` differs from the reader's, **stop and report both versions**. Do not read anything else.
   A different `version_minor` is compatible: minor versions only give meaning to reserved bytes.
4. Map the whole file (`header_bytes + slot_count * (slot_header_bytes + slot_capacity)`).
5. Re-open when `retired` is 1, or when the path now names a different file (the Sender's host restarted).

## Versioning

- `version_major` changes when an existing field moves or changes meaning. Readers refuse other major versions.
- `version_minor` changes when reserved bytes get a meaning. Readers accept any minor version of their major version and must ignore what they do not know.

## What the Feed does not decide

Which frames are **Viewer Frames**. The Sender never feeds thumbnail renders (the host flags them), but Resolve does not flag background cache renders or repeat renders, and the plugin cannot see the playhead. So the Sender feeds every other render, tagged with `frame_time`, `frame_rate`, `render_ns` and `sender_instance`, and the app decides from the pacing (see `app/src/viewer_gate.rs`). Keeping the decision in the app means it can be tested with synthetic frames and changed without touching the Sender.

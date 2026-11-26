# The Feed protocol, version 1.2

The Feed is how the **Sender** (inside Resolve) hands frames to the ScopeCreep app on the same machine. It is one shared-memory file holding a small header and a ring of frame slots. This document is the specification; `include/scopecreep_feed.h` (C, used by the Sender) and the `scopecreep-feed` Rust crate (used by the app) are two implementations of it, tested against each other (`tests/c_interop.rs`).

Think of it as a row of pigeonholes in a corridor between two rooms. The Sender drops a frame in a pigeonhole and walks on without looking up. The app empties pigeonholes when it gets to them. If every pigeonhole is full, the Sender replaces the oldest uncollected frame and makes a tally mark. Nobody ever waits in the corridor.

## Rules that never change

1. **Nobody waits.** No locks, no blocking calls. Every shared field that both sides touch is a lock-free atomic.
2. **One Sender process writes frames, one app reads.** The first running Sender process owns the Feed; Sender processes started while it runs (another Resolve) only register themselves (see "More than one Sender").
3. **A Feed file's layout never changes.** To change the slot count or slot size the Sender makes a new Feed, publishes it in place of the old one (on Linux, renames the new file over the old name; on Windows, see "Windows" below), then marks the old one retired. A reader keeps a valid mapping of the old Feed until it re-opens.
4. **No reader, no work.** The Sender copies frames only while the reader's heartbeat is fresh.
5. All integers are little-endian. All offsets are fixed. Reserved bytes are written as zero and ignored when read.

## The file

The layout below is the same on every platform; only where the Feed lives differs (ADR 0043):

| Platform | Where the Feed lives |
|---|---|
| Linux | the file `/dev/shm/scopecreep-feed` (POSIX shared memory), mode 0600 |
| macOS | the POSIX shared-memory object `/scopecreep-feed` (`shm_open`) |
| Windows | the named file mapping `Local\scopecreep-feed`, backed by the paging file: a locator naming the Feed now published (below) |

`SCOPECREEP_FEED=<path>` overrides the location for both sides on every platform (tests use this); on Windows it is a mapping name such as `Local\scopecreep-feed-test`. Linux and Windows are built (macOS not yet); the Rust crate keeps its platform calls in `feed/src/platform/`, the C header in its `scfeed_os_*` functions.

### Windows

A named mapping can be neither renamed nor replaced while any process holds it, so the name cannot move to a new Feed the way a Linux file name does. Instead the name holds a **locator**, one 4096-byte page, and each Feed is its own mapping beside it:

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0 | 8 | `current` (atomic) | number N of the Feed now published, 0 = none yet |
| 8 | 8 | `last` (atomic) | the last N handed out |

Feed N is the mapping named `<location>.N` (N in decimal, such as `Local\scopecreep-feed.3`), with exactly the layout below.

- **Publishing** (Sender): open or make the locator, take N by incrementing `last`, make `<location>.N` (if that name is still taken, take the next N), fill it in, then store N into `current`. Then mark the previous Feed retired, as on Linux.
- **Opening** (reader): open the locator (missing: there is no Sender), read `current` (0: there is no Sender), open `<location>.N`.
- **Replaced:** the Feed at the location is another one when `current` is no longer the N the reader opened.
- Every process using a Feed holds the locator open too, so it lasts as long as anyone uses a Feed. When the last one closes, the locator and every Feed go: an app started after Resolve quit finds no Feed rather than a stale one.

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
| 12 | 4 | `version_minor` | Sender | 2 |
| 16 | 4 | `header_bytes` | Sender | 4096 |
| 20 | 4 | `slot_header_bytes` | Sender | 4096 |
| 24 | 4 | `slot_count` | Sender | number of slots, 0 = no ring yet |
| 28 | 4 | `retired` (atomic) | Sender | 1 = replaced by a newer Feed file; re-open the path |
| 32 | 8 | `slot_capacity` | Sender | bytes in each slot's pixel area (multiple of 4096) |
| 40 | 8 | `feed_id` | Sender | non-zero, different for every Feed file |
| 48 | 8 | `sender_pid` | Sender | process id of the Sender's host |
| 56 | 8 | reserved | | |
| 64 | 64 | `sender_name` | Sender | UTF-8, NUL padded: `ScopeCreep` |
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
| 768 | 256 | reserved | | |
| 1024 | 3072 | `registry` | every Sender process | 24 entries of 128 bytes listing Sender instances (version 1.2, see "More than one Sender") |

**Clock.** Every `_ns` time is the system-wide monotonic clock in nanoseconds, so both processes can compare times: `CLOCK_MONOTONIC` on Linux, `QueryPerformanceCounter` scaled by `QueryPerformanceFrequency` on Windows. Process ids are the platform's own (on Windows, the process id `OpenProcess` takes).

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
| 1024 | 4 | `colour_setup_provenance` | 0 = none, 1 = project, 2 = manual (see "The Colour Setup block") |
| 1028 | 4 | `colour_setup_bytes` | bytes used in `colour_setup`, at most 3064; 0 when provenance is 0 |
| 1032 | 3064 | `colour_setup` | the Colour Setup the Sender knows for this frame, as UTF-8 `key=value` lines (1.1) |

Pixel values are the Sender's input exactly as Resolve rendered it: unclamped, in the Timeline Colour Space.

### The Colour Setup block (version 1.1)

Version 1.0 reserved these bytes; 1.1 gives them a meaning. A 1.0 Sender writes zeros there, which reads as "no Colour Setup", so readers need no special case.

A Sender cannot learn the project's Colour Setup from Resolve (OFX tells it nothing, ADR 0005), so what it sends is only the Colour Setup it was **given by hand**: the manual override on the Sender node. The project's own Colour Setup reaches the app by another road, from Resolve's scripting API through a helper the app runs (ADR 0006), and never through the Feed.

- **Provenance 0, none**: the Sender has no Colour Setup of its own for this frame. This does *not* mean the frame has no Colour Setup: the app uses the project's.
- **Provenance 2, manual**: set by hand on the Sender node. It wins over the project's, and the app labels it "set manually".
- **Provenance 1, project**: a Colour Setup the Sender learned from the project. No Sender writes this today; it is kept for a Sender that can, and readers treat it as the project's Colour Setup.
- Any other provenance, or a byte count over 3064, reads as none.

`colour_setup` holds `colour_setup_bytes` bytes of UTF-8 text, one `key=value` per line, lines ended by `\n`. Readers trim spaces around keys and values, skip lines with no `=`, and ignore keys they do not know. A writer whose text does not fit cuts it after the last whole line that fits. Keys in 1.1, with values as Resolve names them:

| Key | Meaning | Example values |
|---|---|---|
| `timeline` | Timeline Colour Space | `DaVinci WG/Intermediate`, `ACEScct`, `ACEScc`, `Rec.709 Gamma 2.4` |
| `output` | Output Colour Space (for ACES, the output transform) | `Rec.709 Gamma 2.4`, `P3-D65`, `Rec.2100 ST2084`, `Rec.709 BT.1886`, `Rec.2100 ST2084 (1000 nit)` |
| `levels` | Levels | `video` (legal range) or `full` (data levels) |
| `aces_version` | ACES version, only for ACES timelines | `2.0`, `1.3` |
| `tone_mapping` | output tone mapping (optional) | `None`, `DaVinci` |
| `gamut_mapping` | output gamut mapping (optional) | `None`, `Saturation Compression` |

A missing key is unknown, never a default: in particular a missing `aces_version` on an ACES timeline means unknown, which the app shows as "assumes ACES 2.0".

### Sender identity

Resolve tells an OFX plugin no project name, timeline name or page (Stage 0). So the identity of a frame's Sender in the Feed is what is really available: `sender_pid` and `host_name` (header) plus `sender_instance` and `frame_rate` (slot), and the same four in each registry entry. The app adds the project and timeline names from its scripting helper (ADR 0006); they are not in the Feed.

## More than one Sender (version 1.2)

There can be several Sender instances at once: one per timeline that has the Sender (Resolve creates an instance per timeline and keeps it across timeline and page switches), more if the user puts it on several nodes, and in rare cases instances in a second Resolve process. Exactly one drives the Scopes, chosen by rules both sides can apply:

1. **Between processes, the first one keeps the Feed.** A Sender process publishes a Feed only if the file at the path is missing, is not a version 1.2+ Feed of this major version, is `retired`, belongs to its own process, or belongs to a process that is no longer running. Otherwise it is a **guest**: it maps that Feed's header, registers its instances in the registry and records their renders there, but never copies a frame, never builds a ring and never touches the owner's counters or `sender_error`. About once a second (on a render, without waiting) a guest checks the path again: if the owner published a new Feed file it re-registers there; if the owner has quit it publishes its own Feed (with only its own instances) and from then on sends frames.
2. **Within the owning process, the app chooses** from the frames (`sender_instance`, `render_ns`), because only it sees every frame and the instances do not know about each other's timing:
   - one after another (a timeline switch: one instance stops rendering, another starts) the Feed follows the instance that rendered last, at once; that is not a conflict;
   - in step (two instances rendering the same frames, e.g. on a clip node and on the timeline node: each of their last two renders within half a frame period of the other's) the instance with the lowest number drives, whichever renders first, and stays driving while both keep rendering. The other is flagged. An instance whose first frame comes within half a period of the driver's does not take over before that is settled.
3. **The app flags** every instance that renders in step with the driver, and every guest instance whose `last_render_ns` is under 2 s old in a running process, and names them.

Instances of the owning process that are in step with the driver are still copied into the ring (the app discards their frames). That doubles the copy work while the conflict lasts; the app's notice asks the user to remove the extra Sender.

### Registry entry (128 bytes, at header offset 1024 + 128 × i, i = 0 ... 23)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0 | 4 | `state` (atomic) | 0 free, 1 being filled in, 2 live. Readers ignore entries that are not live. |
| 4 | 4 | `instance` | the instance number within its process (as `sender_instance`) |
| 8 | 8 | `pid` | process id of the instance's host; the Feed's own process if equal to `sender_pid`, else a guest |
| 16 | 8 | `frame_rate` (f64) | frames per second of the instance's timeline, 0 = unknown |
| 24 | 8 | `registered_ns` | monotonic time it registered |
| 32 | 8 | `last_render_ns` (atomic) | monotonic time of its last non-thumbnail render call, fed or not, 0 = none yet |
| 40 | 8 | `last_frame_time` (f64) | source frame number of that render |
| 48 | 8 | `renders` (atomic) | its non-thumbnail render calls so far |
| 56 | 8 | reserved | |
| 64 | 64 | `host_name` | UTF-8, NUL padded: the host name and version of the instance's process |

- **Registering:** look for a live entry with the same `pid` and `instance` (reuse it); else compare-and-swap a free entry's `state` from 0 to 1, fill in the fields, store 2. A full registry is not an error: the instance goes unlisted.
- **Unregistering** (the instance is destroyed): store 0 into `state`.
- **Recording a render:** store `last_frame_time`, add 1 to `renders`, store `last_render_ns` last.
- **A new Feed file** of the same owner (a ring built or replaced) starts with a copy of the old file's live entries at the same places, like the counters.
- Entries of a process that died stay until the Feed file is replaced; readers ignore entries of processes that are not running.

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

1. Open the path (on Windows, through the locator). Missing file: there is no Sender. This is a normal state, not an error.
2. Map the first 4096 bytes. If `magic` does not match, the file is not a Feed.
3. If `version_major` differs from the reader's, **stop and report both versions**. Do not read anything else.
   A different `version_minor` is compatible: minor versions only give meaning to reserved bytes.
4. Map the whole file (`header_bytes + slot_count * (slot_header_bytes + slot_capacity)`).
5. Re-open when `retired` is 1, or when the path now names a different Feed (the Sender's host restarted).

## Versioning

- `version_major` changes when an existing field moves or changes meaning. Readers refuse other major versions.
- `version_minor` changes when reserved bytes get a meaning. Readers accept any minor version of their major version and must ignore what they do not know.
- History: 1.0 the first version (ticket #14). 1.1 the slot's Colour Setup block (ticket #16). 1.2 the Sender registry and guest Senders (ticket #19); an older Feed has no registry (the bytes are zero, so it reads as empty), and a 1.2 Sender treats an older Feed of another process as not live and replaces it, as earlier Senders did.

## What the Feed does not decide

Which frames are **Viewer Frames**. The Sender never feeds thumbnail renders (the host flags them), but Resolve does not flag background cache renders or repeat renders, and the plugin cannot see the playhead. So the Sender feeds every other render, tagged with `frame_time`, `frame_rate`, `render_ns` and `sender_instance`, and the app decides from the pacing (see `app/src/viewer_gate.rs`). Keeping the decision in the app means it can be tested with synthetic frames and changed without touching the Sender.

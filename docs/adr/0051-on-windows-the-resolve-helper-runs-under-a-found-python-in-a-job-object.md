# On Windows the Resolve helper runs under a Python the app finds, and dies with the app through a job object

Date: 2026-11-26 (#250, spec #235). Applies ADR 0006 to Windows.

ADR 0006 runs the Colour Setup helper under Resolve's own Python (`/opt/resolve/bin/ResolvePython` on Linux) and ties it to the app with Linux's parent-death signal. Windows has neither as such. Resolve's scripting README for Windows asks for a Python the user installs, with the scripting module in `%PROGRAMDATA%\Blackmagic Design\DaVinci Resolve\Support\Developer\Scripting\Modules` and `fusionscript.dll` in Resolve's install folder. And Windows has no signal sent to a child when its parent dies.

## Decision

- **Which Python.** `SCOPECREEP_RESOLVE_PYTHON` first, as on Linux. Otherwise the first that exists of: a `ResolvePython.exe` in Resolve's install folder (should Resolve ship one, as on Linux), the Python launcher `py.exe` (`%SystemRoot%`, then the per-user launcher), `python.exe` on `PATH`, and last the `WindowsApps` folder, whose `python.exe` may be Windows' stand-in that only offers the Store. The helper finds the module and library at Resolve's paths, or where `RESOLVE_SCRIPT_API` and `RESOLVE_SCRIPT_LIB` (Resolve's own variables) say. The harness looks in the same places.
- **Dying with the app.** The app makes one job object with kill-on-close, holds its only handle (not inheritable) and never closes it, and puts each helper in it as it starts. When the app's process ends, however it ends, Windows closes the handle and ends every helper in the job. Like a tent on one pole: whoever pulls the pole, the tent comes down. A helper that cannot be put in the job is killed and the Colour Setup says why.
- The platform module's interface is one call, `spawn_tied_to_app`, that starts the helper and ties it on, since a job is joined after the process exists. Linux does exactly what it did (parent-death signal and close-on-exec set before exec, then spawn).

## Considered and rejected

- **Putting the app itself in the job, so its children join at birth:** closes the few microseconds between start and joining, but every process the app starts would die with it, including a browser opened from the Help menu.
- **Shipping a Python with the Installer:** more to build, sign and update, for a feature that already needs Resolve Studio with external scripting.

## Consequences

- On Windows, automatic Colour Setup needs Python 3 (64-bit) installed, unless Resolve ships its own. Without one the Colour Setup line says Resolve's Python could not be started, and the Sender node's manual override still works.
- An app killed in the microseconds between starting a helper and joining it to the job leaves that helper until it next reads its closed stdin.

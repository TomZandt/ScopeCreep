# On Windows the Sender carries its own C++ runtime

Date: 2026-11-26 (#249, spec #235). Applies ADR 0002 to Windows.

ADR 0002 keeps the Sender free of every library Resolve might bring its own copy of. On Linux that means the CUDA runtime and libstdc++ are linked in statically. On Windows the same question is the Visual C++ runtime: link it as DLLs (`/MD`, the MSVC default) and the plugin depends on whichever `vcruntime140.dll` and `msvcp140.dll` Resolve or the system happens to load; link it in (`/MT`) and the plugin carries its own, as `cudart_static.lib` already does. Like packing your own tools for a job on someone else's site instead of borrowing whatever is in their shed.

## Decision

- The Windows Sender links the C++ runtime statically (`/MT`) as well as the CUDA runtime; it imports only Windows' own DLLs. The CTest check `sender_links_only_system_libraries` fails otherwise.
- The Windows build is CMake (`sender/CMakeLists.txt`), since MSVC and the CUDA toolkit are found by CMake without paths; the Linux build stays the Makefile.
- A static runtime keeps its own copy of the environment, taken when the plugin loads. The Sender therefore reads its environment variables (`SCOPECREEP_SENDER_STAGING`, `SCOPECREEP_SENDER_GPU_LIMIT_MB`) from the process through `sender/src/platform.h`, not with `getenv`.

## Considered and rejected

- **`/MD`, the shared runtime:** the plugin's behaviour would depend on the runtime version Resolve ships, and the Installer would have to ship the Visual C++ Redistributable.

## Consequences

- `SCOPECREEP_FEED` is still read with `getenv` by the Feed header; it is set before Resolve starts (or, in the fake-host test, before the plugin loads), so the copy the plugin's runtime takes holds it.

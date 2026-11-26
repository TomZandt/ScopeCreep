# Fails if the built Sender loads any library but the operating system's own (ADR 0002): the CUDA runtime is
# static, no OpenEXR or Imath, and on Windows no Visual C++ runtime DLL. Run by CTest (CMakeLists.txt):
#   cmake -DPLUGIN=<.ofx> -DTOOL=dumpbin|objdump -DTOOL_COMMAND=<path to the tool> -P check_links.cmake
set(CMAKE_GET_RUNTIME_DEPENDENCIES_TOOL "${TOOL}")
set(CMAKE_GET_RUNTIME_DEPENDENCIES_COMMAND "${TOOL_COMMAND}")
if(TOOL STREQUAL "dumpbin")
  set(CMAKE_GET_RUNTIME_DEPENDENCIES_PLATFORM windows+pe)
else()
  set(CMAKE_GET_RUNTIME_DEPENDENCIES_PLATFORM linux+elf)
endif()
file(GET_RUNTIME_DEPENDENCIES
  LIBRARIES "${PLUGIN}"
  RESOLVED_DEPENDENCIES_VAR found
  UNRESOLVED_DEPENDENCIES_VAR missing
  PRE_EXCLUDE_REGEXES "^[Aa][Pp][Ii]-[Mm][Ss]-" "^[Ee][Xx][Tt]-[Mm][Ss]-"
  # The system's folders: Windows\ on Windows, /lib and /usr/lib on Linux.
  POST_EXCLUDE_REGEXES "^[A-Za-z]:[/\\\\][Ww][Ii][Nn][Dd][Oo][Ww][Ss][/\\\\]" "^/lib" "^/usr/lib")
if(found OR missing)
  message(FATAL_ERROR "the Sender must link nothing but system libraries; it also loads: ${found} ${missing}")
endif()
message(STATUS "the Sender links only system libraries")

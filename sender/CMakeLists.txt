# The ScopeCreep Sender's Windows build (MSVC and the CUDA toolkit). Linux builds with the Makefile; this file also
# configures on Linux only so it can be checked on the Linux dev machine. From the repository root:
#
#   cmake -S sender -B sender/build/cmake -DOFX_SDK=<OpenFX include folder>
#   cmake --build sender/build/cmake --config Release          sender/build/scopecreep_sender.ofx.bundle
#   ctest --test-dir sender/build/cmake -C Release --output-on-failure
#                                                              the fake-host test (needs an NVIDIA GPU, no Resolve)
#   cmake --install sender/build/cmake --config Release        developer install (as administrator); restart Resolve
#
# OFX_SDK is Resolve's OpenFX include folder, or the official OpenFX headers plus sender/ci/ofxImageEffectExt.h
# (what CI uses). Same rule as the Makefile (ADR 0002): the CUDA runtime is linked statically and nothing else but
# the operating system's own libraries. On Windows the C++ runtime is static too (/MT, as cudart_static.lib is), so
# the plugin needs no Visual C++ Redistributable; the test `sender_links_only_system_libraries` checks this.
cmake_minimum_required(VERSION 3.21)
project(scopecreep_sender LANGUAGES CXX)
set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreaded)
find_package(CUDAToolkit REQUIRED)

if(WIN32)
  set(platform_dir Win64)
  set(ofx_default "C:/ProgramData/Blackmagic Design/DaVinci Resolve/Support/Developer/OpenFX/OpenFX-1.4/include")
  set(install_default "C:/Program Files/Common Files/OFX/Plugins")
else()
  set(platform_dir Linux-x86-64)
  set(ofx_default /opt/resolve/Developer/OpenFX/OpenFX-1.4/include)
  set(install_default /usr/OFX/Plugins)
endif()
set(OFX_SDK "${ofx_default}" CACHE PATH "OpenFX include folder")
if(NOT EXISTS "${OFX_SDK}/ofxCore.h" OR NOT EXISTS "${OFX_SDK}/ofxImageEffectExt.h")
  message(FATAL_ERROR "No OpenFX headers in OFX_SDK=${OFX_SDK}. Pass -DOFX_SDK=<Resolve's OpenFX include folder>, "
                      "or a folder with the official OpenFX headers and sender/ci/ofxImageEffectExt.h.")
endif()
if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(CMAKE_INSTALL_PREFIX "${install_default}" CACHE PATH "OFX plugin folder" FORCE)
endif()

# The product version, as the Makefile reads it.
file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/../Cargo.toml" version_line REGEX "^version = \"" LIMIT_COUNT 1)
string(REGEX REPLACE "^version = \"([^\"]*)\".*" "\\1" sender_version "${version_line}")

# The bundle goes where the Makefile puts it; `$<0:>` stops Visual Studio adding a Release/ or Debug/ folder.
set(bundle "${CMAKE_CURRENT_SOURCE_DIR}/build/scopecreep_sender.ofx.bundle")
add_library(scopecreep_sender MODULE src/sender.cpp src/feed_copy.cpp)
target_compile_features(scopecreep_sender PRIVATE cxx_std_17)
target_include_directories(scopecreep_sender PRIVATE "${OFX_SDK}" ../feed/include)
target_compile_definitions(scopecreep_sender PRIVATE "SCOPECREEP_SENDER_VERSION=\"${sender_version}\"")
target_link_libraries(scopecreep_sender PRIVATE CUDA::cudart_static)
set_target_properties(scopecreep_sender PROPERTIES
  PREFIX "" SUFFIX ".ofx" CXX_VISIBILITY_PRESET hidden
  LIBRARY_OUTPUT_DIRECTORY "${bundle}/Contents/${platform_dir}$<0:>"
  ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/lib$<0:>")

add_executable(mock_host_test test/mock_host_test.cpp)
target_compile_features(mock_host_test PRIVATE cxx_std_17)
target_include_directories(mock_host_test PRIVATE "${OFX_SDK}" ../feed/include)
target_link_libraries(mock_host_test PRIVATE CUDA::cudart_static ${CMAKE_DL_LIBS})

foreach(target scopecreep_sender mock_host_test)
  if(MSVC)
    # NOMINMAX: windows.h (from the Feed header) must not turn std::max into a macro.
    target_compile_definitions(${target} PRIVATE NOMINMAX _CRT_SECURE_NO_WARNINGS)
    target_compile_options(${target} PRIVATE /W3 /permissive- /utf-8)
  else()
    target_compile_options(${target} PRIVATE -O2 -Wall -Wextra -Wno-unused-parameter)
  endif()
endforeach()
if(NOT WIN32)
  target_link_options(scopecreep_sender PRIVATE -static-libstdc++ -static-libgcc LINKER:--exclude-libs,ALL LINKER:-z,defs)
endif()

install(DIRECTORY "${bundle}" DESTINATION .)

enable_testing()
add_test(NAME sender COMMAND mock_host_test $<TARGET_FILE:scopecreep_sender>)
add_test(NAME sender_guest COMMAND mock_host_test $<TARGET_FILE:scopecreep_sender> --guest)
if(MSVC)
  get_filename_component(linker_dir "${CMAKE_LINKER}" DIRECTORY)
  set(deps_tool dumpbin)
  set(deps_command "${linker_dir}/dumpbin.exe")
else()
  set(deps_tool objdump)
  find_program(deps_command objdump REQUIRED)
endif()
add_test(NAME sender_links_only_system_libraries
         COMMAND "${CMAKE_COMMAND}" -DPLUGIN=$<TARGET_FILE:scopecreep_sender> -DTOOL=${deps_tool} "-DTOOL_COMMAND=${deps_command}"
                 -P "${CMAKE_CURRENT_SOURCE_DIR}/check_links.cmake")
# The tests time render calls against a budget of milliseconds: one at a time.
set_tests_properties(sender sender_guest PROPERTIES RUN_SERIAL TRUE)

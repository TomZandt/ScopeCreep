# The ScopeCreep Sender: an OFX plugin for DaVinci Resolve. Linux (NVIDIA CUDA) and macOS (Apple Silicon, Metal)
# build from this one Makefile; Windows has CMakeLists.txt.
#
#   make            build build/scopecreep_sender.ofx.bundle
#   make test       drive the built plugin with a fake host and check the Feed (needs a GPU, no Resolve)
#   make install    developer install into Resolve's OFX folder; restart Resolve afterwards
#   make uninstall
#   make clean
#
# Linux needs: g++, the CUDA toolkit (CUDA_HOME, default /opt/cuda) and Resolve's OpenFX headers (OFX_SDK).
# macOS needs: clang++ (Xcode command line tools) and Resolve's OpenFX headers (OFX_SDK). `make` on the Mac is the
# build entry the release pipeline calls; it leaves build/scopecreep_sender.ofx.bundle with the plugin in
# Contents/MacOS.
# The plugin links the CUDA runtime statically and nothing else (Linux), or Apple's Metal and Foundation frameworks
# and nothing else (macOS): never add OpenEXR, Imath or any other library Resolve bundles its own copy of (it
# crashed Resolve in Stage 0; ADR 0002).

UNAME := $(shell uname -s)

VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' ../Cargo.toml | head -1)

NAME   := scopecreep_sender
BUNDLE := build/$(NAME).ofx.bundle

# The feed protocol's C header and the shared half of the Sender, whatever the platform.
CORE_SOURCES := src/sender.cpp src/feed_core.cpp
HEADERS      := src/feed_copy.h src/feed_core.h src/platform.h ../feed/include/scopecreep_feed.h

ifeq ($(UNAME),Darwin)
# ------------------------------------------------------------------------------------------------ macOS, Metal
OFX_SDK        ?= /Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/OpenFX/OpenFX-1.4/include
OFX_PLUGIN_DIR ?= /Library/OFX/Plugins

PLUGIN := $(BUNDLE)/Contents/MacOS/$(NAME).ofx

CXX      ?= clang++
# arm64 only (Apple Silicon); Intel Macs and universal binaries are out of scope. The .mm file is compiled with ARC.
COMMON   := -std=c++17 -O2 -fvisibility=hidden -Wall -Wextra -Wno-unused-parameter -arch arm64 -mmacosx-version-min=12.0 \
            -I"$(OFX_SDK)" -I../feed/include -DSCOPECREEP_SENDER_VERSION='"$(VERSION)"'
FRAMEWORKS := -framework Metal -framework Foundation

OBJECTS := build/obj/sender.o build/obj/feed_core.o build/obj/feed_metal.o

all: $(PLUGIN)

# Rebuilt whenever the flags change.
build/flags: FORCE
	@mkdir -p build
	@echo '$(COMMON)' | cmp -s - $@ || echo '$(COMMON)' > $@

build/obj/%.o: src/%.cpp $(HEADERS) build/flags
	@mkdir -p $(dir $@)
	$(CXX) $(COMMON) -c $< -o $@

build/obj/feed_metal.o: src/feed_metal.mm $(HEADERS) build/flags
	@mkdir -p $(dir $@)
	$(CXX) $(COMMON) -fobjc-arc -x objective-c++ -c $< -o $@

$(PLUGIN): $(OBJECTS) macos/Info.plist
	@mkdir -p $(dir $@)
	$(CXX) -arch arm64 -mmacosx-version-min=12.0 -bundle $(OBJECTS) -o $@ $(FRAMEWORKS)
	sed 's/@VERSION@/$(VERSION)/g' macos/Info.plist > $(BUNDLE)/Contents/Info.plist
	codesign --force --sign - $(BUNDLE)
	@echo "Built $(BUNDLE). Check: file $(PLUGIN)  (expect: Mach-O 64-bit bundle arm64)"

build/mock_host_test: test/mock_host_test.cpp test/test_gpu.h test/test_gpu_metal.mm test/test_platform.h ../feed/include/scopecreep_feed.h
	@mkdir -p build/obj
	$(CXX) -std=c++17 -O1 -Wall -arch arm64 -I"$(OFX_SDK)" -I../feed/include -c test/mock_host_test.cpp -o build/obj/mock_host_test.o
	$(CXX) -std=c++17 -O1 -Wall -arch arm64 -fobjc-arc -I"$(OFX_SDK)" -I../feed/include -x objective-c++ -c test/test_gpu_metal.mm -o build/obj/test_gpu_metal.o
	$(CXX) -arch arm64 build/obj/mock_host_test.o build/obj/test_gpu_metal.o -o $@ $(FRAMEWORKS)

test: $(PLUGIN) build/mock_host_test
	@lipo -archs $(PLUGIN) | grep -qx arm64 || { echo "the Sender must be arm64 only"; exit 1; }
	@! otool -L $(PLUGIN) | tail -n +2 | grep -v -E '^[[:space:]]+(/System/Library/|/usr/lib/)' || { echo "the Sender may link only system libraries and frameworks"; exit 1; }
	./build/mock_host_test $(PLUGIN)
	./build/mock_host_test $(PLUGIN) --guest

install: $(PLUGIN)
	sudo mkdir -p "$(OFX_PLUGIN_DIR)"
	sudo rm -rf "$(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle"
	sudo cp -R $(BUNDLE) "$(OFX_PLUGIN_DIR)/"
	@echo "Installed $(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle. Resolve scans OFX plugins at start-up: quit it (Cmd-Q) and start it again."

uninstall:
	sudo rm -rf "$(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle"

else
# --------------------------------------------------------------------------------------------- Linux, CUDA
CUDA_HOME      ?= /opt/cuda
OFX_SDK        ?= /opt/resolve/Developer/OpenFX/OpenFX-1.4/include
OFX_PLUGIN_DIR ?= /usr/OFX/Plugins

PLUGIN := $(BUNDLE)/Contents/Linux-x86-64/$(NAME).ofx

CXX      ?= g++
CXXFLAGS := -std=c++17 -O2 -fPIC -fvisibility=hidden -Wall -Wextra -Wno-unused-parameter \
            -I$(OFX_SDK) -I$(CUDA_HOME)/include -I../feed/include -DSCOPECREEP_SENDER_VERSION='"$(VERSION)"'
CUDA_LIBS := -L$(CUDA_HOME)/lib64 -lcudart_static -ldl -lrt -lpthread

SOURCES := $(CORE_SOURCES) src/feed_cuda.cpp

all: $(PLUGIN)

# Rebuilt whenever the flags change.
build/flags: FORCE
	@mkdir -p build
	@echo '$(CXXFLAGS)' | cmp -s - $@ || echo '$(CXXFLAGS)' > $@

$(PLUGIN): $(SOURCES) $(HEADERS) build/flags
	@mkdir -p $(dir $@)
	$(CXX) $(CXXFLAGS) -shared $(SOURCES) -o $@ $(CUDA_LIBS) -static-libstdc++ -static-libgcc -Wl,--exclude-libs,ALL -Wl,-z,defs

build/mock_host_test: test/mock_host_test.cpp test/test_gpu.h test/test_gpu_cuda.cpp test/test_platform.h ../feed/include/scopecreep_feed.h
	@mkdir -p build
	$(CXX) -std=c++17 -O1 -Wall -I$(OFX_SDK) -I$(CUDA_HOME)/include -I../feed/include test/mock_host_test.cpp test/test_gpu_cuda.cpp -o $@ $(CUDA_LIBS)

test: $(PLUGIN) build/mock_host_test
	@! ldd $(PLUGIN) | grep -i -E 'openexr|imath|iex|ilmthread' || (echo "the Sender must not link OpenEXR or Imath"; exit 1)
	./build/mock_host_test $(PLUGIN)
	./build/mock_host_test $(PLUGIN) --guest

install: $(PLUGIN)
	@test -d "$(OFX_PLUGIN_DIR)" && test -w "$(OFX_PLUGIN_DIR)" || { \
	  echo "$(OFX_PLUGIN_DIR) is missing or not writable. Create it once:"; \
	  echo "  sudo mkdir -p $(OFX_PLUGIN_DIR) && sudo chown $$USER $(OFX_PLUGIN_DIR)"; exit 1; }
	rm -rf "$(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle"
	cp -r $(BUNDLE) "$(OFX_PLUGIN_DIR)/"
	@echo "Installed $(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle. Resolve scans OFX plugins at startup: restart Resolve."

uninstall:
	rm -rf "$(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle"
endif

clean:
	rm -rf build

FORCE:
.PHONY: all test install uninstall clean FORCE

# The ScopeCreep Sender: an OFX plugin for DaVinci Resolve (Linux, NVIDIA CUDA).
#
#   make            build build/scopecreep_sender.ofx.bundle
#   make test       drive the built plugin with a fake CUDA host and check the Feed (needs an NVIDIA GPU, no Resolve)
#   make install    developer install into Resolve's OFX folder; restart Resolve afterwards
#   make uninstall
#   make clean
#
# Needs: g++, the CUDA toolkit (CUDA_HOME, default /opt/cuda) and Resolve's OpenFX headers (OFX_SDK).
# The plugin links the CUDA runtime statically and nothing else: never add OpenEXR, Imath or any other library
# Resolve bundles its own copy of (it crashed Resolve in Stage 0; ADR 0002).

CUDA_HOME      ?= /opt/cuda
OFX_SDK        ?= /opt/resolve/Developer/OpenFX/OpenFX-1.4/include
OFX_PLUGIN_DIR ?= /usr/OFX/Plugins
VERSION        := $(shell sed -n 's/^version = "\(.*\)"/\1/p' ../Cargo.toml | head -1)

NAME   := scopecreep_sender
BUNDLE := build/$(NAME).ofx.bundle
PLUGIN := $(BUNDLE)/Contents/Linux-x86-64/$(NAME).ofx

CXX      ?= g++
CXXFLAGS := -std=c++17 -O2 -fPIC -fvisibility=hidden -Wall -Wextra -Wno-unused-parameter \
            -I$(OFX_SDK) -I$(CUDA_HOME)/include -I../feed/include -DSCOPECREEP_SENDER_VERSION='"$(VERSION)"'
CUDA_LIBS := -L$(CUDA_HOME)/lib64 -lcudart_static -ldl -lrt -lpthread

SOURCES := src/sender.cpp src/feed_copy.cpp
HEADERS := src/feed_copy.h ../feed/include/scopecreep_feed.h

all: $(PLUGIN)

# Rebuilt whenever the flags change.
build/flags: FORCE
	@mkdir -p build
	@echo '$(CXXFLAGS)' | cmp -s - $@ || echo '$(CXXFLAGS)' > $@

$(PLUGIN): $(SOURCES) $(HEADERS) build/flags
	@mkdir -p $(dir $@)
	$(CXX) $(CXXFLAGS) -shared $(SOURCES) -o $@ $(CUDA_LIBS) -static-libstdc++ -static-libgcc -Wl,--exclude-libs,ALL -Wl,-z,defs

build/mock_host_test: test/mock_host_test.cpp ../feed/include/scopecreep_feed.h
	@mkdir -p build
	$(CXX) -std=c++17 -O1 -Wall -I$(OFX_SDK) -I$(CUDA_HOME)/include -I../feed/include $< -o $@ $(CUDA_LIBS)

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

clean:
	rm -rf build

FORCE:
.PHONY: all test install uninstall clean FORCE

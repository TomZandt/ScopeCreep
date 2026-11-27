# PROTOTYPE (throwaway): macOS arm64 Metal OFX probe, issue #253.
#   make        build build/scopecreep_metal_probe.ofx.bundle (arm64) and ad-hoc sign it
#   make test   offline test of the pure helpers (any OS)
#   make install / uninstall   into /Library/OFX/Plugins (needs sudo)
#   make clean
# OFX_SDK: Resolve's OpenFX headers. If your Resolve keeps them elsewhere, or you cloned
# https://github.com/AcademySoftwareFoundation/openfx, pass OFX_SDK=<dir containing ofxCore.h>.

OFX_SDK ?= /Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/OpenFX/OpenFX-1.4/include
OFX_PLUGIN_DIR ?= /Library/OFX/Plugins

NAME   := scopecreep_metal_probe
BUNDLE := build/$(NAME).ofx.bundle
PLUGIN := $(BUNDLE)/Contents/MacOS/$(NAME).ofx

CXX      ?= clang++
CXXFLAGS := -std=c++17 -O2 -fobjc-arc -fvisibility=hidden -Wall -arch arm64 -mmacosx-version-min=12.0 -I"$(OFX_SDK)"

all: $(PLUGIN)

$(PLUGIN): src/probe_metal.mm src/probe_format.h Info.plist
	@mkdir -p $(dir $@)
	$(CXX) $(CXXFLAGS) -x objective-c++ -bundle src/probe_metal.mm -o $@ -framework Metal -framework Foundation
	cp Info.plist $(BUNDLE)/Contents/Info.plist
	codesign --force --sign - $(BUNDLE)
	@echo "Built $(BUNDLE). Check: file $(PLUGIN)  (expect: Mach-O 64-bit bundle arm64)"

build/format_test: test/format_test.cpp src/probe_format.h
	@mkdir -p build
	$(CXX) -std=c++17 -Wall test/format_test.cpp -o $@

test: build/format_test
	./build/format_test

install: $(PLUGIN)
	sudo mkdir -p "$(OFX_PLUGIN_DIR)"
	sudo rm -rf "$(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle"
	sudo cp -R $(BUNDLE) "$(OFX_PLUGIN_DIR)/"
	@echo "Installed. Quit Resolve completely (Cmd-Q) and start it again: it scans OFX plugins at start-up."

uninstall:
	sudo rm -rf "$(OFX_PLUGIN_DIR)/$(NAME).ofx.bundle"

clean:
	rm -rf build

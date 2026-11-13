# PROTOTYPE (throwaway): Stage 0 probe Sender, issue #4.
#   make            build scopecreep_probe.ofx.bundle
#   make install    copy the bundle into Resolve's Linux OFX folder
#   make test       EXR round-trip test
#   make clean
# Override OFX_PLUGIN_DIR if needed. CUDA is not used by the probe;
# CUDAPATH is only used by `make cuda-check` (toolchain smoke test).

SDK        ?= /opt/resolve/Developer/OpenFX
CUDAPATH   ?= /opt/cuda
OFX_PLUGIN_DIR ?= /usr/OFX/Plugins

NAME   = scopecreep_probe
BUNDLE = $(NAME).ofx.bundle
BIN    = $(BUNDLE)/Contents/Linux-x86-64/$(NAME).ofx

CXX      ?= g++
CXXFLAGS = -std=c++17 -O2 -fPIC -fvisibility=hidden -Wall -I$(SDK)/OpenFX-1.4/include \
           -I/usr/include/OpenEXR -I/usr/include/Imath
LDLIBS   = -lOpenEXR -lOpenEXRCore -lImath -lIex -lpthread
# The plugin links NO OpenEXR (clashes with Resolve's bundled copy); only the test uses it.

all: $(BIN)

$(BIN): src/probe_sender.cpp src/exr_dump.cpp src/exr_dump.h
	mkdir -p $(dir $@)
	$(CXX) $(CXXFLAGS) -shared src/probe_sender.cpp src/exr_dump.cpp -o $@ -lpthread

build/exr_test: test/exr_roundtrip_test.cpp src/exr_dump.cpp src/exr_dump.h
	mkdir -p build
	$(CXX) $(CXXFLAGS) test/exr_roundtrip_test.cpp src/exr_dump.cpp -o $@ $(LDLIBS)

build/mock_host_test: test/mock_host_test.cpp
	mkdir -p build
	$(CXX) $(CXXFLAGS) test/mock_host_test.cpp -o $@ -ldl

# Offline checks (no Resolve): EXR bit-exact round trip, and the built plugin
# driven by a fake host (passthrough, logging, trigger-file dump).
test: build/exr_test build/mock_host_test $(BIN)
	./build/exr_test
	./build/mock_host_test $(BIN)

install: $(BIN)
	@if [ ! -d "$(OFX_PLUGIN_DIR)" ]; then \
	  echo "ERROR: $(OFX_PLUGIN_DIR) does not exist. Create it once: sudo mkdir -p $(OFX_PLUGIN_DIR) && sudo chown $$USER $(OFX_PLUGIN_DIR)"; exit 1; fi
	rm -rf $(OFX_PLUGIN_DIR)/$(BUNDLE)
	cp -r $(BUNDLE) $(OFX_PLUGIN_DIR)/
	@echo "Installed. Resolve rescans OFX plugins only at startup: restart Resolve."

cuda-check:
	@echo '__global__ void k(){}  int main(){return 0;}' > build_cuda_check.cu
	$(CUDAPATH)/bin/nvcc build_cuda_check.cu -o build_cuda_check && ./build_cuda_check && echo "CUDA toolchain OK"
	@rm -f build_cuda_check build_cuda_check.cu

clean:
	rm -rf $(BUNDLE) build

.PHONY: all test install cuda-check clean

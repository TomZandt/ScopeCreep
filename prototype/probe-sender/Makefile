# PROTOTYPE (throwaway): Stage 0 probe Sender, issue #4.
#   make            build scopecreep_probe.ofx.bundle
#   make install    copy the bundle into Resolve's Linux OFX folder
#   make test       EXR round-trip test
#   make clean
# Override OFX_PLUGIN_DIR if needed.
# Issue #12: the plugin now contains CUDA code (src/cuda_copy.cu, linked with static cudart; used only
# when run with SCOPECREEP_PROBE_CUDA=1). Also: build/probe_reader (outside Resolve), `make cuda-test`.

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

NVCC      = $(CUDAPATH)/bin/nvcc
NVCCFLAGS = -O2 -std=c++17 -gencode arch=compute_86,code=sm_86 --compiler-options="-fPIC -fvisibility=hidden"

all: $(BIN) build/probe_reader

build/cuda_copy.o: src/cuda_copy.cu src/cuda_copy.h src/ring_layout.h
	mkdir -p build
	$(NVCC) $(NVCCFLAGS) -c $< -o $@

$(BIN): src/probe_sender.cpp src/exr_dump.cpp src/exr_dump.h src/cuda_copy.h src/ring_layout.h build/cuda_copy.o
	mkdir -p $(dir $@)
	$(CXX) $(CXXFLAGS) -shared src/probe_sender.cpp src/exr_dump.cpp build/cuda_copy.o -o $@ \
	    -L$(CUDAPATH)/lib64 -lcudart_static -lrt -ldl -lpthread

build/probe_reader: src/probe_reader.cpp src/ring_layout.h
	mkdir -p build
	$(CXX) -std=c++17 -O2 -Wall src/probe_reader.cpp -o $@ -lrt -lpthread

build/cuda_copy_test: test/cuda_copy_test.cu build/cuda_copy.o
	mkdir -p build
	$(NVCC) $(NVCCFLAGS) test/cuda_copy_test.cu build/cuda_copy.o -o $@ -lrt

# Offline end-to-end check of the async copy + ring + reader (no Resolve): a fake renderer at 24 fps and flat out,
# staged and direct, reader verifying GPU checksums on every frame.
cuda-test: build/cuda_copy_test build/probe_reader
	./build/cuda_copy_test 24 6 staged /scopecreep-test-ring & sleep 1; ./build/probe_reader --verify --seconds 5 --shm /scopecreep-test-ring; wait
	./build/cuda_copy_test 24 6 direct /scopecreep-test-ring & sleep 1; ./build/probe_reader --verify --seconds 5 --shm /scopecreep-test-ring; wait
	./build/cuda_copy_test 0 6 staged /scopecreep-test-ring & sleep 1; ./build/probe_reader --verify --sleep-ms 40 --seconds 5 --shm /scopecreep-test-ring; wait

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

.PHONY: all test cuda-test install cuda-check clean

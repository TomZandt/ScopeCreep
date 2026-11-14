# The Sender is C++; the app is Rust with wgpu

The Sender is C++ because Resolve's OFX SDK, samples and GPU paths (CUDA, OpenCL, Metal) all target C++. Its only job is to hand frames over, which keeps the C++ code small. The app is Rust with wgpu, which gives one compute-shader codebase across Metal, Vulkan and DirectX 12, plus memory safety in the larger codebase. The two talk through a small, language-neutral shared-memory protocol.

## Considered Options

- C++ with Qt 6 throughout: one language, but clumsier GPU compute and LGPL or commercial licensing constraints for a sold product.
- C++ with Vulkan and MoltenVK: maximum control, most work.

## Consequences

- The Sender must never link OpenEXR or Imath. Resolve bundles its own copies; linking the system ones crashed Resolve in Stage 0. The Sender stays free of heavyweight dependencies.

// PROTOTYPE (throwaway): Stage 0 check 6 (issue #12). Layout of the shared-memory frame ring
// shared by the probe plugin (writer, inside Resolve) and probe_reader (outside Resolve).
//
// File: shm_open("/scopecreep-probe-ring") = /dev/shm/scopecreep-probe-ring
//   [RingHeader : 4096 bytes][slot 0][slot 1]...   slot = [SlotHeader : 4096 bytes][pixel data : slotBytes]
// Pixel data is 32-bit float RGBA, rows bottom-up exactly as OFX hands them over, tightly packed
// (rowBytes = width*16).
//
// Protocol (single logical writer, one reader, nobody ever waits):
//   writer: take a FREE slot; if none, take the oldest READY slot (a drop is counted); if none
//           (all WRITING/READING), skip this frame (a drop is counted). FREE/READY -> WRITING,
//           async GPU copy, completion callback -> READY (release store).
//   reader: pick the READY slot with the lowest seq, READY -> READING (CAS), use it in place
//           (the writer never touches WRITING or READING slots), READING -> FREE.
#pragma once
#include <atomic>
#include <cstddef>
#include <cstdint>

namespace sc {

constexpr uint64_t kRingMagic = 0x53434f5045524e47ull;  // "SCOPERNG"
constexpr uint32_t kRingVersion = 1;
constexpr size_t kHeaderBytes = 4096;

enum SlotState : uint32_t { kFree = 0, kWriting = 1, kReady = 2, kReading = 3 };

struct RingHeader {
    uint64_t magic;
    uint32_t version;
    uint32_t nslots;
    uint64_t slotBytes;                 // capacity of the pixel data area of one slot
    std::atomic<uint64_t> nextSeq;      // next sequence number to hand out
    std::atomic<uint64_t> published;    // copies completed
    std::atomic<uint64_t> dropOverwrite;// oldest unread READY slot overwritten
    std::atomic<uint64_t> dropNoSlot;   // all slots busy: frame skipped
    std::atomic<uint64_t> dropNoStaging;// GPU staging buffer still busy: frame skipped
    std::atomic<uint64_t> dropOversize; // frame larger than slotBytes
    std::atomic<uint64_t> skippedThumb; // thumbnail renders not copied
};

struct SlotHeader {
    std::atomic<uint32_t> state;
    uint32_t inst;
    uint64_t seq;
    double time;                        // OFX render time of the frame
    uint32_t width, height;
    uint32_t hasChecksum;
    uint32_t pad;
    uint64_t checksum;                  // GPU-computed frame checksum (see frameChecksum)
    double submitWallMs;                // steady-clock ms at copy submission
    double completeWallMs;              // steady-clock ms when the host callback saw completion
};

inline size_t slotStride(uint64_t slotBytes) { return kHeaderBytes + (size_t)slotBytes; }
inline size_t ringBytes(uint32_t nslots, uint64_t slotBytes) { return kHeaderBytes + (size_t)nslots * slotStride(slotBytes); }
inline SlotHeader* slotHeader(void* base, uint32_t i, uint64_t slotBytes)
{ return (SlotHeader*)((char*)base + kHeaderBytes + (size_t)i * slotStride(slotBytes)); }
inline void* slotData(void* base, uint32_t i, uint64_t slotBytes)
{ return (char*)base + kHeaderBytes + (size_t)i * slotStride(slotBytes) + kHeaderBytes; }

// Order-independent checksum over the 32-bit words of the frame: sum(word * (2*index+1)) mod 2^64.
// Computed on the GPU (atomicAdd) and recomputed by the reader on the CPU.
inline uint64_t frameChecksum(const uint32_t* w, size_t nwords)
{
    uint64_t s = 0;
    for (size_t i = 0; i < nwords; ++i) s += (uint64_t)w[i] * (2ull * i + 1ull);
    return s;
}

}  // namespace sc

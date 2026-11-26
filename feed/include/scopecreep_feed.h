/* The Feed protocol, version 1.2: C implementation (header only, C11 and C++17; Linux with GCC or Clang, Windows
 * with MSVC or Clang).
 * The specification is feed/PROTOCOL.md; keep the two in step. Used by the Sender (writer side) and by
 * feed/c/feed_c_tool.c (both sides, for the interop tests against the Rust crate).
 *
 * Nothing here blocks: no locks, no waits. Shared fields are accessed with lock-free atomic builtins. */
#ifndef SCOPECREEP_FEED_H
#define SCOPECREEP_FEED_H

#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <intrin.h>
#else
#include <signal.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define SCFEED_MAGIC 0x0000444545464353ull /* "SCFEED\0\0" read as a little-endian u64 */
#define SCFEED_VERSION_MAJOR 1u
#define SCFEED_VERSION_MINOR 2u /* 1.1: the slot's Colour Setup block has a defined format; 1.2: the Sender registry */
#define SCFEED_HEADER_BYTES 4096u
#define SCFEED_SLOT_HEADER_BYTES 4096u
/* Where the Feed lives (feed/PROTOCOL.md, ADR 0043). Linux: a file in /dev/shm. Windows: the named file mapping
 * "Local\scopecreep-feed", which holds the locator of the Feed now published (PROTOCOL.md, "Windows").
 * macOS (not built yet): shm_open("/scopecreep-feed"). */
#ifdef _WIN32
#define SCFEED_DEFAULT_PATH "Local\\scopecreep-feed"
#define SCFEED_LOCATOR_BYTES 4096u
#else
#define SCFEED_DEFAULT_PATH "/dev/shm/scopecreep-feed"
#endif
#define SCFEED_PATH_ENV "SCOPECREEP_FEED"
#define SCFEED_SENDER_NAME "ScopeCreep"
#define SCFEED_READER_TIMEOUT_NS 1000000000ull
#define SCFEED_COLOUR_SETUP_BYTES 3064u
#define SCFEED_REGISTRY_ENTRIES 24u

enum { SCFEED_ENTRY_FREE = 0, SCFEED_ENTRY_CLAIMING = 1, SCFEED_ENTRY_LIVE = 2 };
enum { SCFEED_FREE = 0, SCFEED_WRITING = 1, SCFEED_READY = 2, SCFEED_READING = 3 };
enum { SCFEED_PIXEL_RGBA32F = 1 };
enum { SCFEED_ROWS_BOTTOM_UP = 0, SCFEED_ROWS_TOP_DOWN = 1 };
enum { SCFEED_PROVENANCE_NONE = 0, SCFEED_PROVENANCE_PROJECT = 1, SCFEED_PROVENANCE_MANUAL = 2 };
enum { SCFEED_OK = 0, SCFEED_ERR_NO_FEED = -1, SCFEED_ERR_NOT_A_FEED = -2, SCFEED_ERR_VERSION = -3, SCFEED_ERR_BUSY = -4, SCFEED_ERR_IO = -5 };

/* One Sender instance in the registry (version 1.2). 128 bytes. */
typedef struct scfeed_entry {
    uint32_t state;          /*   0 atomic: SCFEED_ENTRY_* */
    uint32_t instance;       /*   4 */
    uint64_t pid;            /*   8 */
    double frame_rate;       /*  16 */
    uint64_t registered_ns;  /*  24 */
    uint64_t last_render_ns; /*  32 atomic */
    double last_frame_time;  /*  40 */
    uint64_t renders;        /*  48 atomic */
    uint64_t reserved;       /*  56 */
    char host_name[64];      /*  64 */
} scfeed_entry;

typedef struct scfeed_header {
    uint64_t magic;                /*   0 */
    uint32_t version_major;        /*   8 */
    uint32_t version_minor;        /*  12 */
    uint32_t header_bytes;         /*  16 */
    uint32_t slot_header_bytes;    /*  20 */
    uint32_t slot_count;           /*  24 */
    uint32_t retired;              /*  28 atomic */
    uint64_t slot_capacity;        /*  32 */
    uint64_t feed_id;              /*  40 */
    uint64_t sender_pid;           /*  48 */
    uint64_t reserved0;            /*  56 */
    char sender_name[64];          /*  64 */
    char sender_version[32];       /* 128 */
    char host_name[64];            /* 160 */
    uint8_t reserved1[32];         /* 224 */
    uint64_t reader_heartbeat_ns;  /* 256 atomic, written by the reader */
    uint64_t reader_pid;           /* 264 atomic, written by the reader */
    uint8_t reserved2[48];         /* 272 */
    uint64_t next_sequence;        /* 320 atomic */
    uint64_t frames_published;     /* 328 atomic */
    uint64_t drops_overwritten;    /* 336 atomic */
    uint64_t drops_busy;           /* 344 atomic */
    uint64_t renders_total;        /* 352 atomic */
    uint64_t renders_thumbnail;    /* 360 atomic */
    uint64_t renders_idle;         /* 368 atomic */
    uint64_t renders_unsupported;  /* 376 atomic */
    uint64_t render_call_ns_total; /* 384 atomic */
    uint64_t last_render_ns;       /* 392 atomic */
    uint8_t reserved3[112];        /* 400 */
    char sender_error[256];        /* 512 */
    uint8_t reserved4[256];        /* 768 */
    scfeed_entry registry[SCFEED_REGISTRY_ENTRIES]; /* 1024, version 1.2 */
} scfeed_header;

typedef struct scfeed_slot {
    uint32_t state;                   /*    0 atomic */
    uint32_t pixel_format;            /*    4 */
    uint64_t sequence;                /*    8 atomic */
    uint32_t width;                   /*   16 */
    uint32_t height;                  /*   20 */
    uint32_t row_bytes;               /*   24 */
    uint32_t row_order;               /*   28 */
    uint64_t data_bytes;              /*   32 */
    double frame_time;                /*   40 */
    double frame_rate;                /*   48 */
    uint64_t render_ns;               /*   56 */
    uint64_t publish_ns;              /*   64 */
    uint32_t sender_instance;         /*   72 */
    uint32_t flags;                   /*   76 */
    uint8_t reserved0[944];           /*   80 */
    uint32_t colour_setup_provenance; /* 1024 */
    uint32_t colour_setup_bytes;      /* 1028 */
    uint8_t colour_setup[SCFEED_COLOUR_SETUP_BYTES]; /* 1032 */
} scfeed_slot;

#ifdef __cplusplus
#define SCFEED_ASSERT(c, m) static_assert(c, m)
#else
#define SCFEED_ASSERT(c, m) _Static_assert(c, m)
#endif
SCFEED_ASSERT(sizeof(scfeed_header) == 4096, "Feed header is 4096 bytes");
SCFEED_ASSERT(offsetof(scfeed_header, retired) == 28, "retired");
SCFEED_ASSERT(offsetof(scfeed_header, sender_name) == 64, "sender_name");
SCFEED_ASSERT(offsetof(scfeed_header, host_name) == 160, "host_name");
SCFEED_ASSERT(offsetof(scfeed_header, reader_heartbeat_ns) == 256, "reader_heartbeat_ns");
SCFEED_ASSERT(offsetof(scfeed_header, next_sequence) == 320, "next_sequence");
SCFEED_ASSERT(offsetof(scfeed_header, last_render_ns) == 392, "last_render_ns");
SCFEED_ASSERT(offsetof(scfeed_header, sender_error) == 512, "sender_error");
SCFEED_ASSERT(sizeof(scfeed_entry) == 128, "registry entry is 128 bytes");
SCFEED_ASSERT(offsetof(scfeed_header, registry) == 1024, "registry");
SCFEED_ASSERT(offsetof(scfeed_entry, last_render_ns) == 32, "last_render_ns");
SCFEED_ASSERT(offsetof(scfeed_entry, host_name) == 64, "host_name");
SCFEED_ASSERT(sizeof(scfeed_slot) == 4096, "slot header is 4096 bytes");
SCFEED_ASSERT(offsetof(scfeed_slot, sequence) == 8, "sequence");
SCFEED_ASSERT(offsetof(scfeed_slot, frame_time) == 40, "frame_time");
SCFEED_ASSERT(offsetof(scfeed_slot, sender_instance) == 72, "sender_instance");
SCFEED_ASSERT(offsetof(scfeed_slot, colour_setup_provenance) == 1024, "colour_setup_provenance");

/* ------------------------------------------------------------------ helpers */

/* The system-wide monotonic clock in nanoseconds, the clock of every _ns field: CLOCK_MONOTONIC on Linux,
 * QueryPerformanceCounter on Windows. */
static inline uint64_t scfeed_now_ns(void)
{
#ifdef _WIN32
    LARGE_INTEGER f, c;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    const uint64_t freq = f.QuadPart > 0 ? (uint64_t)f.QuadPart : 1, count = (uint64_t)c.QuadPart;
    return count / freq * 1000000000ull + count % freq * 1000000000ull / freq;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
#endif
}

/* This process's id, as the Feed records it. */
static inline uint64_t scfeed_pid(void)
{
#ifdef _WIN32
    return (uint64_t)GetCurrentProcessId();
#else
    return (uint64_t)getpid();
#endif
}

/* The Feed path: SCOPECREEP_FEED if set, else the default. */
static inline const char* scfeed_path(void)
{
    const char* p = getenv(SCFEED_PATH_ENV);
    return p && *p ? p : SCFEED_DEFAULT_PATH;
}

#if defined(_MSC_VER) && !defined(__clang__)
/* MSVC has no __atomic builtins: the Interlocked intrinsics are full barriers, stronger than the orders below. */
static inline uint64_t scfeed_load64(const uint64_t* p) { return (uint64_t)_InterlockedOr64((volatile __int64*)p, 0); }
static inline void scfeed_store64(uint64_t* p, uint64_t v) { _InterlockedExchange64((volatile __int64*)p, (__int64)v); }
static inline void scfeed_add64(uint64_t* p, uint64_t v) { _InterlockedExchangeAdd64((volatile __int64*)p, (__int64)v); }
static inline uint64_t scfeed_inc64(uint64_t* p) { return (uint64_t)_InterlockedIncrement64((volatile __int64*)p); }
static inline uint32_t scfeed_load32(const uint32_t* p) { return (uint32_t)_InterlockedOr((volatile long*)p, 0); }
static inline void scfeed_store32(uint32_t* p, uint32_t v) { _InterlockedExchange((volatile long*)p, (long)v); }
static inline int scfeed_cas32(uint32_t* p, uint32_t expect, uint32_t v)
{
    return (uint32_t)_InterlockedCompareExchange((volatile long*)p, (long)v, (long)expect) == expect;
}
#else
static inline uint64_t scfeed_load64(const uint64_t* p) { return __atomic_load_n(p, __ATOMIC_ACQUIRE); }
static inline void scfeed_store64(uint64_t* p, uint64_t v) { __atomic_store_n(p, v, __ATOMIC_RELEASE); }
static inline void scfeed_add64(uint64_t* p, uint64_t v) { __atomic_fetch_add(p, v, __ATOMIC_RELAXED); }
/* Add one and return the new value. */
static inline uint64_t scfeed_inc64(uint64_t* p) { return __atomic_add_fetch(p, 1, __ATOMIC_RELAXED); }
static inline uint32_t scfeed_load32(const uint32_t* p) { return __atomic_load_n(p, __ATOMIC_ACQUIRE); }
static inline void scfeed_store32(uint32_t* p, uint32_t v) { __atomic_store_n(p, v, __ATOMIC_RELEASE); }
static inline int scfeed_cas32(uint32_t* p, uint32_t expect, uint32_t v)
{
    return __atomic_compare_exchange_n(p, &expect, v, 0, __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE);
}
#endif

static inline size_t scfeed_slot_stride(const scfeed_header* h) { return (size_t)h->slot_header_bytes + (size_t)h->slot_capacity; }
static inline size_t scfeed_file_bytes(const scfeed_header* h) { return (size_t)h->header_bytes + (size_t)h->slot_count * scfeed_slot_stride(h); }
static inline scfeed_slot* scfeed_slot_at(scfeed_header* h, uint32_t i)
{
    return (scfeed_slot*)((uint8_t*)h + h->header_bytes + (size_t)i * scfeed_slot_stride(h));
}
static inline void* scfeed_slot_pixels(scfeed_header* h, uint32_t i) { return (uint8_t*)scfeed_slot_at(h, i) + h->slot_header_bytes; }

/* Is a reader attached? (Its heartbeat is less than a second old.) */
static inline int scfeed_reader_present(const scfeed_header* h, uint64_t now_ns)
{
    const uint64_t hb = scfeed_load64(&h->reader_heartbeat_ns);
    return hb != 0 && (now_ns < hb || now_ns - hb < SCFEED_READER_TIMEOUT_NS);
}

static inline void scfeed_set_text(char* dst, size_t cap, const char* src)
{
    memset(dst, 0, cap);
    if (src) strncpy(dst, src, cap - 1);
}

/* Is process `pid` running? Linux: signal 0 checks only; EPERM means it runs as someone else. Windows: a process
 * still running is not signalled; one we may not open (another user's, or elevated) exists. */
static inline int scfeed_process_alive(uint64_t pid)
{
#ifdef _WIN32
    if (pid == 0 || pid > 0xffffffffull) return 0;
    HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, (DWORD)pid);
    if (!h) return GetLastError() == ERROR_ACCESS_DENIED;
    const int alive = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
    CloseHandle(h);
    return alive;
#else
    return pid != 0 && pid <= 0x7fffffffull && (kill((pid_t)pid, 0) == 0 || errno == EPERM);
#endif
}

/* ------------------------------------------ where the Feed lives, per OS */
/* One Feed's shared memory while this process opens or makes it. A new Feed is made out of sight and then
 * published at the path in one step, so a reader only ever opens a whole one. Linux: a file, made under a
 * temporary name and renamed over the path. Windows: a named mapping can be neither renamed nor replaced while
 * anyone holds it, so the path names a locator page (offset 0: the published Feed's number N, 0 = none;
 * offset 8: the last N handed out) and Feed N is the mapping "<path>.N" (PROTOCOL.md, "Windows"). */

#ifdef _WIN32
typedef struct scfeed_os_feed {
    HANDLE locator, section;
    size_t size;
    uint64_t n;
} scfeed_os_feed;

static inline void scfeed_os_close(scfeed_os_feed* f)
{
    if (f->section) CloseHandle(f->section);
    if (f->locator) CloseHandle(f->locator);
    f->section = f->locator = NULL;
}
static inline uint64_t* scfeed_os_locator(scfeed_os_feed* f)
{
    return (uint64_t*)MapViewOfFile(f->locator, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, SCFEED_LOCATOR_BYTES);
}
static inline void scfeed_os_name(char* out, size_t cap, const char* path, uint64_t n)
{
    snprintf(out, cap, "%s.%llu", path, (unsigned long long)n);
}

/* Open the Feed published at `path`. SCFEED_OK, SCFEED_ERR_NO_FEED or SCFEED_ERR_IO. */
static inline int scfeed_os_open(scfeed_os_feed* f, const char* path)
{
    memset(f, 0, sizeof *f);
    f->locator = OpenFileMappingA(FILE_MAP_READ | FILE_MAP_WRITE, FALSE, path);
    if (!f->locator) return GetLastError() == ERROR_FILE_NOT_FOUND ? SCFEED_ERR_NO_FEED : SCFEED_ERR_IO;
    uint64_t* l = scfeed_os_locator(f);
    if (!l) { scfeed_os_close(f); return SCFEED_ERR_IO; }
    f->n = scfeed_load64(&l[0]);
    UnmapViewOfFile(l);
    if (!f->n) { scfeed_os_close(f); return SCFEED_ERR_NO_FEED; }
    char name[600];
    scfeed_os_name(name, sizeof name, path, f->n);
    f->section = OpenFileMappingA(FILE_MAP_READ | FILE_MAP_WRITE, FALSE, name);
    if (!f->section) {
        const DWORD e = GetLastError();
        scfeed_os_close(f);
        return e == ERROR_FILE_NOT_FOUND ? SCFEED_ERR_NO_FEED : SCFEED_ERR_IO;
    }
    /* Its size: map all of it and ask how large the view is (a Feed is whole pages). */
    void* all = MapViewOfFile(f->section, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, 0);
    MEMORY_BASIC_INFORMATION info;
    const int ok = all && VirtualQuery(all, &info, sizeof info) != 0;
    if (all) UnmapViewOfFile(all);
    if (!ok) { scfeed_os_close(f); return SCFEED_ERR_IO; }
    f->size = info.RegionSize;
    return SCFEED_OK;
}

/* Make a new Feed of `bytes` bytes in the paging file, not yet published at `path`. SCFEED_OK or SCFEED_ERR_IO. */
static inline int scfeed_os_create(scfeed_os_feed* f, const char* path, size_t bytes)
{
    memset(f, 0, sizeof *f);
    f->locator = CreateFileMappingA(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE, 0, SCFEED_LOCATOR_BYTES, path);
    if (!f->locator) return SCFEED_ERR_IO;
    uint64_t* l = scfeed_os_locator(f);
    if (!l) { scfeed_os_close(f); return SCFEED_ERR_IO; }
    for (int attempt = 0; attempt < 16 && !f->section; ++attempt) {
        f->n = scfeed_inc64(&l[1]);
        char name[600];
        scfeed_os_name(name, sizeof name, path, f->n);
        SetLastError(0);
        HANDLE s = CreateFileMappingA(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE, (DWORD)((uint64_t)bytes >> 32), (DWORD)bytes, name);
        if (!s) break;
        if (GetLastError() == ERROR_ALREADY_EXISTS) CloseHandle(s); /* left from an earlier locator: take the next N */
        else f->section = s;
    }
    UnmapViewOfFile(l);
    if (!f->section) { scfeed_os_close(f); return SCFEED_ERR_IO; }
    f->size = bytes;
    return SCFEED_OK;
}

static inline void* scfeed_os_map(scfeed_os_feed* f, size_t len)
{
    return MapViewOfFile(f->section, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, len);
}
static inline void scfeed_os_unmap(void* p, size_t len) { (void)len; UnmapViewOfFile(p); }

/* Make the Feed from scfeed_os_create the one at `path`. */
static inline int scfeed_os_publish(scfeed_os_feed* f, const char* path)
{
    (void)path;
    uint64_t* l = scfeed_os_locator(f);
    if (!l) return SCFEED_ERR_IO;
    scfeed_store64(&l[0], f->n);
    UnmapViewOfFile(l);
    return SCFEED_OK;
}
/* A Feed from scfeed_os_create that will not be published. */
static inline void scfeed_os_discard(scfeed_os_feed* f) { scfeed_os_close(f); }
/* The writer has mapped its Feed: keep the handles, since a mapping's name goes with its last handle. */
static inline void scfeed_os_keep(scfeed_os_feed* f) { (void)f; }
/* The writer is done with its Feed. */
static inline void scfeed_os_release(scfeed_os_feed* f) { scfeed_os_close(f); }
#else
typedef struct scfeed_os_feed {
    int fd;
    size_t size;
    char tmp[600]; /* a new Feed's temporary name */
} scfeed_os_feed;

static inline void scfeed_os_close(scfeed_os_feed* f)
{
    if (f->fd >= 0) close(f->fd);
    f->fd = -1;
}

/* Open the Feed at `path`. SCFEED_OK, SCFEED_ERR_NO_FEED or SCFEED_ERR_IO. */
static inline int scfeed_os_open(scfeed_os_feed* f, const char* path)
{
    memset(f, 0, sizeof *f);
    f->fd = open(path, O_RDWR | O_CLOEXEC);
    if (f->fd < 0) return errno == ENOENT ? SCFEED_ERR_NO_FEED : SCFEED_ERR_IO;
    struct stat st;
    if (fstat(f->fd, &st) != 0) { scfeed_os_close(f); return SCFEED_ERR_IO; }
    f->size = (size_t)st.st_size;
    return SCFEED_OK;
}

/* Make a new Feed of `bytes` bytes, not yet published at `path`. SCFEED_OK or SCFEED_ERR_IO (errno set). */
static inline int scfeed_os_create(scfeed_os_feed* f, const char* path, size_t bytes)
{
    memset(f, 0, sizeof *f);
    snprintf(f->tmp, sizeof f->tmp, "%s.%ld.tmp", path, (long)getpid());
    unlink(f->tmp);
    f->fd = open(f->tmp, O_RDWR | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (f->fd < 0) return SCFEED_ERR_IO;
    if (ftruncate(f->fd, (off_t)bytes) != 0) { scfeed_os_close(f); unlink(f->tmp); return SCFEED_ERR_IO; }
    f->size = bytes;
    return SCFEED_OK;
}

static inline void* scfeed_os_map(scfeed_os_feed* f, size_t len)
{
    void* p = mmap(NULL, len, PROT_READ | PROT_WRITE, MAP_SHARED, f->fd, 0);
    return p == MAP_FAILED ? NULL : p;
}
static inline void scfeed_os_unmap(void* p, size_t len) { munmap(p, len); }

/* Make the Feed from scfeed_os_create the one at `path`. SCFEED_OK or SCFEED_ERR_IO (errno set). */
static inline int scfeed_os_publish(scfeed_os_feed* f, const char* path)
{
    return rename(f->tmp, path) == 0 ? SCFEED_OK : SCFEED_ERR_IO;
}
/* A Feed from scfeed_os_create that will not be published. */
static inline void scfeed_os_discard(scfeed_os_feed* f)
{
    const int e = errno;
    scfeed_os_close(f);
    unlink(f->tmp);
    errno = e;
}
/* The writer has mapped its Feed: the file keeps its name by itself, so the descriptor can go. */
static inline void scfeed_os_keep(scfeed_os_feed* f) { scfeed_os_close(f); }
/* The writer is done with its Feed. */
static inline void scfeed_os_release(scfeed_os_feed* f) { (void)f; }
#endif

/* ------------------------------------------------------------ the registry */
/* Every Sender instance lists itself here: instances of the Feed's own process (`sender_pid`) and guests from
 * other Sender processes, which send no frames (PROTOCOL.md, "More than one Sender"). Nothing here waits. */

static inline int scfeed_registry_find(const scfeed_header* h, uint64_t pid, uint32_t instance)
{
    for (uint32_t i = 0; i < SCFEED_REGISTRY_ENTRIES; ++i) {
        const scfeed_entry* e = &h->registry[i];
        if (scfeed_load32(&e->state) == SCFEED_ENTRY_LIVE && e->pid == pid && e->instance == instance) return (int)i;
    }
    return -1;
}

/* List an instance. Returns its entry index, or -1 if the registry is full (not an error: it goes unlisted). */
static inline int scfeed_registry_add(scfeed_header* h, uint64_t pid, uint32_t instance, double frame_rate, const char* host_name)
{
    int i = scfeed_registry_find(h, pid, instance);
    if (i >= 0) return i;
    for (uint32_t k = 0; k < SCFEED_REGISTRY_ENTRIES; ++k) {
        scfeed_entry* e = &h->registry[k];
        if (!scfeed_cas32(&e->state, SCFEED_ENTRY_FREE, SCFEED_ENTRY_CLAIMING)) continue;
        e->instance = instance;
        e->pid = pid;
        e->frame_rate = frame_rate;
        e->registered_ns = scfeed_now_ns();
        scfeed_store64(&e->last_render_ns, 0);
        e->last_frame_time = 0;
        scfeed_store64(&e->renders, 0);
        scfeed_set_text(e->host_name, sizeof e->host_name, host_name);
        scfeed_store32(&e->state, SCFEED_ENTRY_LIVE);
        return (int)k;
    }
    return -1;
}

static inline void scfeed_registry_remove(scfeed_header* h, uint64_t pid, uint32_t instance)
{
    const int i = scfeed_registry_find(h, pid, instance);
    if (i >= 0) scfeed_store32(&h->registry[i].state, SCFEED_ENTRY_FREE);
}

/* Record one render call of an instance (fed or not; thumbnails excluded). */
static inline void scfeed_registry_note(scfeed_header* h, uint64_t pid, uint32_t instance, double frame_time, uint64_t render_ns)
{
    const int i = scfeed_registry_find(h, pid, instance);
    if (i < 0) return;
    scfeed_entry* e = &h->registry[i];
    e->last_frame_time = frame_time;
    scfeed_add64(&e->renders, 1);
    scfeed_store64(&e->last_render_ns, render_ns);
}

/* Copy the live entries of `from` (all of them, or only those of process `only_pid` if it is not 0) into a new
 * header `to` that nobody else uses yet, at the same places. */
static inline void scfeed_registry_copy(scfeed_header* to, const scfeed_header* from, uint64_t only_pid)
{
    if (from->version_minor < 2) return;
    for (uint32_t i = 0; i < SCFEED_REGISTRY_ENTRIES; ++i) {
        const scfeed_entry* e = &from->registry[i];
        if (scfeed_load32(&e->state) != SCFEED_ENTRY_LIVE || (only_pid && e->pid != only_pid)) continue;
        memcpy(&to->registry[i], e, sizeof *e);
        to->registry[i].state = SCFEED_ENTRY_LIVE;
    }
}

/* Unmap a header from scfeed_live_feed_of_another. */
static inline void scfeed_header_unmap(scfeed_header* h) { scfeed_os_unmap(h, SCFEED_HEADER_BYTES); }

/* If the Feed at `path` is a live Feed (this major version, minor >= 1, not retired) of a running process other
 * than `my_pid`, map its header and return it (unmap with scfeed_header_unmap): a Sender starting in `my_pid`
 * must join it as a guest. Otherwise NULL: the Sender may publish its own Feed. */
static inline scfeed_header* scfeed_live_feed_of_another(const char* path, uint64_t my_pid)
{
    scfeed_os_feed f;
    if (scfeed_os_open(&f, path) != SCFEED_OK) return NULL;
    void* m = f.size >= SCFEED_HEADER_BYTES ? scfeed_os_map(&f, SCFEED_HEADER_BYTES) : NULL;
    scfeed_os_close(&f);
    if (!m) return NULL;
    scfeed_header* h = (scfeed_header*)m;
    if (h->magic == SCFEED_MAGIC && h->version_major == SCFEED_VERSION_MAJOR && h->version_minor >= 2 && !scfeed_load32(&h->retired) &&
        h->sender_pid != my_pid && scfeed_process_alive(h->sender_pid))
        return h;
    scfeed_header_unmap(h);
    return NULL;
}

/* ------------------------------------------------------------- writer side */

/* Put a Colour Setup (UTF-8 `key=value` lines, see PROTOCOL.md) in a slot the writer owns, or clear it with
 * SCFEED_PROVENANCE_NONE. Text longer than the block is cut after its last whole line that fits. */
static inline void scfeed_slot_set_colour_setup(scfeed_slot* s, uint32_t provenance, const char* text, size_t len)
{
    if (provenance == SCFEED_PROVENANCE_NONE || !text) {
        s->colour_setup_provenance = SCFEED_PROVENANCE_NONE;
        s->colour_setup_bytes = 0;
        return;
    }
    if (len > SCFEED_COLOUR_SETUP_BYTES) {
        len = SCFEED_COLOUR_SETUP_BYTES;
        while (len > 0 && text[len - 1] != '\n') --len;
    }
    memcpy(s->colour_setup, text, len);
    s->colour_setup_bytes = (uint32_t)len;
    s->colour_setup_provenance = provenance;
}

typedef struct scfeed_writer {
    scfeed_header* hdr; /* the whole mapping starts here */
    size_t bytes;
    char path[512];
    scfeed_os_feed os; /* what the OS needs kept while the Feed is in use */
} scfeed_writer;

/* Publish a new Feed at `path` with `slot_count` slots of `slot_capacity` bytes (rounded up to 4096).
 * The Feed is built out of sight and then published at `path`; a Feed already there is marked retired.
 * Returns SCFEED_OK or SCFEED_ERR_IO (errno set on Linux). */
static inline int scfeed_writer_create(scfeed_writer* w, const char* path, uint32_t slot_count, uint64_t slot_capacity,
                                       const char* sender_version, const char* host_name)
{
    memset(w, 0, sizeof *w);
    slot_capacity = (slot_capacity + 4095ull) & ~4095ull;
    const size_t bytes = SCFEED_HEADER_BYTES + (size_t)slot_count * (SCFEED_SLOT_HEADER_BYTES + (size_t)slot_capacity);
    if (scfeed_os_create(&w->os, path, bytes) != SCFEED_OK) return SCFEED_ERR_IO;
    void* base = scfeed_os_map(&w->os, bytes);
    if (!base) { scfeed_os_discard(&w->os); return SCFEED_ERR_IO; }
    scfeed_os_keep(&w->os);
    scfeed_header* h = (scfeed_header*)base; /* a new Feed is all zeros */
    h->version_major = SCFEED_VERSION_MAJOR;
    h->version_minor = SCFEED_VERSION_MINOR;
    h->header_bytes = SCFEED_HEADER_BYTES;
    h->slot_header_bytes = SCFEED_SLOT_HEADER_BYTES;
    h->slot_count = slot_count;
    h->slot_capacity = slot_capacity;
    h->feed_id = (scfeed_now_ns() << 16) ^ scfeed_pid() ^ (uint64_t)(uintptr_t)base;
    if (h->feed_id == 0) h->feed_id = 1;
    h->sender_pid = scfeed_pid();
    scfeed_set_text(h->sender_name, sizeof h->sender_name, SCFEED_SENDER_NAME);
    scfeed_set_text(h->sender_version, sizeof h->sender_version, sender_version);
    scfeed_set_text(h->host_name, sizeof h->host_name, host_name);
    h->magic = SCFEED_MAGIC;

    /* Find the Feed that is there now, so it can be told it has been replaced. */
    scfeed_header* old = NULL;
    scfeed_os_feed of;
    if (scfeed_os_open(&of, path) == SCFEED_OK) {
        if (of.size >= SCFEED_HEADER_BYTES) old = (scfeed_header*)scfeed_os_map(&of, SCFEED_HEADER_BYTES);
        scfeed_os_close(&of);
    }
    if (scfeed_os_publish(&w->os, path) != SCFEED_OK) {
        const int e = errno;
        if (old) scfeed_header_unmap(old);
        scfeed_os_unmap(base, bytes);
        scfeed_os_discard(&w->os);
        errno = e;
        return SCFEED_ERR_IO;
    }
    if (old) {
        if (old->magic == SCFEED_MAGIC) scfeed_store32(&old->retired, 1);
        scfeed_header_unmap(old);
    }
    w->hdr = h;
    w->bytes = bytes;
    snprintf(w->path, sizeof w->path, "%s", path);
    return SCFEED_OK;
}

/* Unmap. The Feed stays while a reader is still using it. */
static inline void scfeed_writer_close(scfeed_writer* w)
{
    if (w->hdr) {
        scfeed_os_unmap(w->hdr, w->bytes);
        scfeed_os_release(&w->os);
    }
    w->hdr = NULL;
}

/* Take a slot for a new frame without waiting: a FREE one, else the oldest READY one (a drop is counted).
 * Returns the slot index with the slot in WRITING state and its `sequence` set, or -1 if the frame must be
 * skipped (a drop is counted). */
static inline int scfeed_writer_acquire(scfeed_writer* w)
{
    scfeed_header* h = w->hdr;
    int taken = -1;
    for (int attempt = 0; attempt < 2 && taken < 0; ++attempt) {
        for (uint32_t i = 0; i < h->slot_count; ++i)
            if (scfeed_cas32(&scfeed_slot_at(h, i)->state, SCFEED_FREE, SCFEED_WRITING)) { taken = (int)i; break; }
        if (taken >= 0) break;
        int oldest = -1;
        uint64_t oldest_seq = ~0ull;
        for (uint32_t i = 0; i < h->slot_count; ++i) {
            scfeed_slot* s = scfeed_slot_at(h, i);
            if (scfeed_load32(&s->state) == SCFEED_READY && scfeed_load64(&s->sequence) < oldest_seq) {
                oldest_seq = scfeed_load64(&s->sequence);
                oldest = (int)i;
            }
        }
        if (oldest < 0) break;
        if (scfeed_cas32(&scfeed_slot_at(h, (uint32_t)oldest)->state, SCFEED_READY, SCFEED_WRITING)) {
            scfeed_add64(&h->drops_overwritten, 1);
            taken = oldest;
        } /* else the reader took it between the scan and the swap: look once more */
    }
    if (taken < 0) { scfeed_add64(&h->drops_busy, 1); return -1; }
    scfeed_store64(&scfeed_slot_at(h, (uint32_t)taken)->sequence, scfeed_inc64(&h->next_sequence));
    return taken;
}

/* The copy into slot `i` has completed: make the frame visible to the reader. */
static inline void scfeed_writer_publish(scfeed_writer* w, int i)
{
    scfeed_slot* s = scfeed_slot_at(w->hdr, (uint32_t)i);
    s->publish_ns = scfeed_now_ns();
    scfeed_add64(&w->hdr->frames_published, 1);
    scfeed_store32(&s->state, SCFEED_READY);
}

/* The copy into slot `i` could not be started: give the slot back and count the frame as dropped. */
static inline void scfeed_writer_abandon(scfeed_writer* w, int i)
{
    scfeed_add64(&w->hdr->drops_busy, 1);
    scfeed_store32(&scfeed_slot_at(w->hdr, (uint32_t)i)->state, SCFEED_FREE);
}

/* ------------------------------------------------------------- reader side */

typedef struct scfeed_reader {
    scfeed_header* hdr;
    size_t bytes;
    uint32_t found_major, found_minor; /* filled in even when the version is refused */
} scfeed_reader;

static inline void scfeed_reader_heartbeat(scfeed_reader* r, uint64_t now_ns)
{
    scfeed_store64(&r->hdr->reader_pid, scfeed_pid());
    scfeed_store64(&r->hdr->reader_heartbeat_ns, now_ns);
}

/* Attach to the Feed at `path`. Returns SCFEED_OK or one of the SCFEED_ERR_ codes. */
static inline int scfeed_reader_open(scfeed_reader* r, const char* path)
{
    memset(r, 0, sizeof *r);
    scfeed_os_feed f;
    const int opened = scfeed_os_open(&f, path);
    if (opened != SCFEED_OK) return opened;
    if (f.size < SCFEED_HEADER_BYTES) { scfeed_os_close(&f); return SCFEED_ERR_NOT_A_FEED; }
    scfeed_header* h = (scfeed_header*)scfeed_os_map(&f, SCFEED_HEADER_BYTES);
    if (!h) { scfeed_os_close(&f); return SCFEED_ERR_IO; }
    int rc = SCFEED_OK;
    size_t bytes = 0;
    if (h->magic != SCFEED_MAGIC) rc = SCFEED_ERR_NOT_A_FEED;
    else {
        r->found_major = h->version_major;
        r->found_minor = h->version_minor;
        if (h->version_major != SCFEED_VERSION_MAJOR) rc = SCFEED_ERR_VERSION;
        else {
            bytes = scfeed_file_bytes(h);
            if (f.size < bytes) rc = SCFEED_ERR_NOT_A_FEED;
            else if (scfeed_reader_present(h, scfeed_now_ns()) && scfeed_load64(&h->reader_pid) != scfeed_pid()) rc = SCFEED_ERR_BUSY;
        }
    }
    scfeed_header_unmap(h);
    if (rc != SCFEED_OK) { scfeed_os_close(&f); return rc; }
    void* base = scfeed_os_map(&f, bytes);
    scfeed_os_close(&f);
    if (!base) return SCFEED_ERR_IO;
    r->hdr = (scfeed_header*)base;
    r->bytes = bytes;
    for (uint32_t i = 0; i < r->hdr->slot_count; ++i) /* slots left READING belong to a reader that died */
        scfeed_cas32(&scfeed_slot_at(r->hdr, i)->state, SCFEED_READING, SCFEED_FREE);
    scfeed_reader_heartbeat(r, scfeed_now_ns());
    return SCFEED_OK;
}

/* Detach cleanly: the Sender stops copying at once. */
static inline void scfeed_reader_close(scfeed_reader* r)
{
    if (!r->hdr) return;
    scfeed_store64(&r->hdr->reader_heartbeat_ns, 0);
    scfeed_os_unmap(r->hdr, r->bytes);
    r->hdr = NULL;
}

/* Claim the oldest unread frame. Returns its slot index (state READING) or -1 if there is none. */
static inline int scfeed_reader_claim(scfeed_reader* r)
{
    scfeed_header* h = r->hdr;
    for (int attempt = 0; attempt < 4; ++attempt) {
        int oldest = -1;
        uint64_t oldest_seq = ~0ull;
        for (uint32_t i = 0; i < h->slot_count; ++i) {
            scfeed_slot* s = scfeed_slot_at(h, i);
            if (scfeed_load32(&s->state) == SCFEED_READY && scfeed_load64(&s->sequence) < oldest_seq) {
                oldest_seq = scfeed_load64(&s->sequence);
                oldest = (int)i;
            }
        }
        if (oldest < 0) return -1;
        if (scfeed_cas32(&scfeed_slot_at(h, (uint32_t)oldest)->state, SCFEED_READY, SCFEED_READING)) return oldest;
    }
    return -1;
}

static inline void scfeed_reader_release(scfeed_reader* r, int i) { scfeed_store32(&scfeed_slot_at(r->hdr, (uint32_t)i)->state, SCFEED_FREE); }

#ifdef __cplusplus
}
#endif
#endif

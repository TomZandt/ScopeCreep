/* Drives the C implementation of the Feed protocol (scopecreep_feed.h) from the command line, so the Rust
 * crate's tests can check the two implementations against each other (feed/tests/c_interop.rs). Not shipped.
 *
 *   feed_c_tool write <path> <slots> <width> <height> <frames>   publish a Feed and write frames as the Sender would
 *   feed_c_tool write-version <path> <major> <minor>             publish an empty Feed claiming another version
 *   feed_c_tool read <path>                                      attach as a reader and print every waiting frame
 *   feed_c_tool serve <path> <width> <height> <ms>               act as a Sender for <ms>: copy only while a reader is attached
 *   feed_c_tool write-colour <path> <provenance> <text>          write two 16x9 frames, the second with this Colour Setup
 *   feed_c_tool read-colour <path>                               print the Colour Setup of every waiting frame
 *   feed_c_tool guest <path> <renders>                           start as a Sender process does (join a live Feed of another
 *                                                                process as a guest, else publish), print the role, record renders
 *   feed_c_tool registry <path>                                  print the live registry entries
 * The writing commands register their frames' Sender instance (7, or 3 for `guest`) in the registry.
 */
#include "scopecreep_feed.h"

#include <stdlib.h>

static void fill_slot(scfeed_writer* w, int i, uint32_t width, uint32_t height, uint64_t f)
{
    scfeed_slot* s = scfeed_slot_at(w->hdr, (uint32_t)i);
    s->pixel_format = SCFEED_PIXEL_RGBA32F;
    s->width = width;
    s->height = height;
    s->row_bytes = width * 16;
    s->row_order = SCFEED_ROWS_BOTTOM_UP;
    s->data_bytes = (uint64_t)width * 16 * height;
    s->frame_time = 100.0 + (double)f;
    s->frame_rate = 25.0;
    s->render_ns = scfeed_now_ns();
    s->sender_instance = 7;
    float* px = (float*)scfeed_slot_pixels(w->hdr, (uint32_t)i);
    for (uint32_t k = 0; k < width * height * 4; ++k) px[k] = (float)f * 1000.0f + (float)k * 0.25f - 2.0f;
}

static int cmd_write(const char* path, uint32_t slots, uint32_t width, uint32_t height, uint64_t frames)
{
    scfeed_writer w;
    if (scfeed_writer_create(&w, path, slots, (uint64_t)width * height * 16, "c-tool", "feed_c_tool") != SCFEED_OK) { perror("create"); return 1; }
    scfeed_registry_add(w.hdr, (uint64_t)getpid(), 7, 25.0, "feed_c_tool");
    for (uint64_t f = 0; f < frames; ++f) {
        scfeed_registry_note(w.hdr, (uint64_t)getpid(), 7, 100.0 + (double)f, scfeed_now_ns());
        int i = scfeed_writer_acquire(&w);
        if (i < 0) continue;
        fill_slot(&w, i, width, height, f);
        scfeed_writer_publish(&w, i);
    }
    scfeed_writer_close(&w);
    return 0;
}

static int cmd_read(const char* path)
{
    scfeed_reader r;
    int rc = scfeed_reader_open(&r, path);
    if (rc == SCFEED_ERR_NO_FEED) { printf("no feed\n"); return 0; }
    if (rc == SCFEED_ERR_VERSION) { printf("version mismatch: found %u.%u\n", r.found_major, r.found_minor); return 0; }
    if (rc != SCFEED_OK) { printf("error %d\n", rc); return 0; }
    printf("sender=%s slots=%u\n", r.hdr->sender_name, r.hdr->slot_count);
    int n = 0, i;
    while ((i = scfeed_reader_claim(&r)) >= 0) {
        scfeed_slot* s = scfeed_slot_at(r.hdr, (uint32_t)i);
        const float* px = (const float*)scfeed_slot_pixels(r.hdr, (uint32_t)i);
        const size_t count = (size_t)(s->data_bytes / 4);
        double sum = 0;
        for (size_t k = 0; k < count; ++k) sum += px[k];
        printf("%llu %u %u %u %u %.3f %.3f %u %.1f %.1f %.1f\n", (unsigned long long)s->sequence, s->width, s->height, s->row_bytes,
               s->row_order, s->frame_time, s->frame_rate, s->sender_instance, px[0], px[count - 1], sum);
        scfeed_reader_release(&r, i);
        ++n;
    }
    printf("frames=%d\n", n);
    scfeed_reader_close(&r);
    return 0;
}

static int cmd_write_colour(const char* path, uint32_t provenance, const char* text)
{
    scfeed_writer w;
    if (scfeed_writer_create(&w, path, 4, 16 * 9 * 16, "c-tool", "feed_c_tool") != SCFEED_OK) { perror("create"); return 1; }
    for (uint64_t f = 0; f < 2; ++f) {
        int i = scfeed_writer_acquire(&w);
        if (i < 0) return 1;
        fill_slot(&w, i, 16, 9, f);
        scfeed_slot* s = scfeed_slot_at(w.hdr, (uint32_t)i);
        if (f == 0) scfeed_slot_set_colour_setup(s, SCFEED_PROVENANCE_NONE, NULL, 0);
        else scfeed_slot_set_colour_setup(s, provenance, text, strlen(text));
        scfeed_writer_publish(&w, i);
    }
    scfeed_writer_close(&w);
    return 0;
}

static int cmd_read_colour(const char* path)
{
    scfeed_reader r;
    if (scfeed_reader_open(&r, path) != SCFEED_OK) { printf("cannot open\n"); return 1; }
    int i;
    while ((i = scfeed_reader_claim(&r)) >= 0) {
        const scfeed_slot* s = scfeed_slot_at(r.hdr, (uint32_t)i);
        printf("provenance=%u bytes=%u\n%.*s", s->colour_setup_provenance, s->colour_setup_bytes, (int)s->colour_setup_bytes, (const char*)s->colour_setup);
        scfeed_reader_release(&r, i);
    }
    scfeed_reader_close(&r);
    return 0;
}

static int cmd_serve(const char* path, uint32_t width, uint32_t height, uint64_t ms)
{
    scfeed_writer w;
    if (scfeed_writer_create(&w, path, 4, (uint64_t)width * height * 16, "c-tool", "feed_c_tool") != SCFEED_OK) { perror("create"); return 1; }
    const uint64_t end = scfeed_now_ns() + ms * 1000000ull;
    uint64_t f = 0;
    while (scfeed_now_ns() < end) {
        scfeed_add64(&w.hdr->renders_total, 1);
        if (!scfeed_reader_present(w.hdr, scfeed_now_ns())) {
            scfeed_add64(&w.hdr->renders_idle, 1); /* no reader, no work */
        } else {
            int i = scfeed_writer_acquire(&w);
            if (i >= 0) {
                fill_slot(&w, i, width, height, f++);
                scfeed_writer_publish(&w, i);
            }
        }
        usleep(5000);
    }
    printf("published=%llu idle=%llu\n", (unsigned long long)w.hdr->frames_published, (unsigned long long)w.hdr->renders_idle);
    scfeed_writer_close(&w);
    return 0;
}

static int cmd_guest(const char* path, int renders)
{
    const uint64_t me = (uint64_t)getpid();
    scfeed_os_feed held;
    scfeed_header* owner = scfeed_live_feed_of_another(path, me, &held);
    scfeed_writer w;
    scfeed_header* h = owner;
    if (!owner) {
        if (scfeed_writer_create(&w, path, 0, 0, "c-tool", "feed_c_tool") != SCFEED_OK) { perror("create"); return 1; }
        h = w.hdr;
    }
    printf("%s\n", owner ? "guest" : "owner");
    scfeed_registry_add(h, me, 3, 25.0, "feed_c_tool");
    for (int k = 0; k < renders; ++k) scfeed_registry_note(h, me, 3, (double)k, scfeed_now_ns());
    if (owner) scfeed_header_unmap(owner, &held);
    else scfeed_writer_close(&w);
    return 0;
}

static int cmd_registry(const char* path)
{
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) { perror("open"); return 1; }
    const scfeed_header* h = (const scfeed_header*)mmap(NULL, SCFEED_HEADER_BYTES, PROT_READ, MAP_SHARED, fd, 0);
    close(fd);
    if (h == MAP_FAILED) { perror("mmap"); return 1; }
    for (uint32_t i = 0; i < SCFEED_REGISTRY_ENTRIES; ++i) {
        const scfeed_entry* e = &h->registry[i];
        if (scfeed_load32(&e->state) != SCFEED_ENTRY_LIVE) continue;
        printf("%llu %u %.3f %llu %.3f %llu %s\n", (unsigned long long)e->pid, e->instance, e->frame_rate, (unsigned long long)e->last_render_ns,
               e->last_frame_time, (unsigned long long)e->renders, e->host_name);
    }
    return 0;
}

int main(int argc, char** argv)
{
    if (argc == 4 && !strcmp(argv[1], "guest")) return cmd_guest(argv[2], atoi(argv[3]));
    if (argc == 3 && !strcmp(argv[1], "registry")) return cmd_registry(argv[2]);
    if (argc == 7 && !strcmp(argv[1], "write"))
        return cmd_write(argv[2], (uint32_t)atoi(argv[3]), (uint32_t)atoi(argv[4]), (uint32_t)atoi(argv[5]), (uint64_t)atoll(argv[6]));
    if (argc == 5 && !strcmp(argv[1], "write-version")) {
        scfeed_writer w;
        if (scfeed_writer_create(&w, argv[2], 0, 0, "c-tool", "feed_c_tool") != SCFEED_OK) { perror("create"); return 1; }
        w.hdr->version_major = (uint32_t)atoi(argv[3]);
        w.hdr->version_minor = (uint32_t)atoi(argv[4]);
        return 0;
    }
    if (argc == 3 && !strcmp(argv[1], "read")) return cmd_read(argv[2]);
    if (argc == 5 && !strcmp(argv[1], "write-colour")) return cmd_write_colour(argv[2], (uint32_t)atoi(argv[3]), argv[4]);
    if (argc == 3 && !strcmp(argv[1], "read-colour")) return cmd_read_colour(argv[2]);
    if (argc == 6 && !strcmp(argv[1], "serve")) return cmd_serve(argv[2], (uint32_t)atoi(argv[3]), (uint32_t)atoi(argv[4]), (uint64_t)atoll(argv[5]));
    fprintf(stderr, "usage: see the top of feed_c_tool.c\n");
    return 2;
}

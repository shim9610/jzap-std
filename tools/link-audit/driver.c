/* Single-threaded failure-injection allocator and entry point. libc is a trusted
 * boundary; --wrap catches direct exit calls from the linked Rust objects. */
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
static size_t calls, fail_at, live;
void *audit_alloc(size_t size, size_t align) {
    if (++calls == fail_at) return NULL;
    void *p = NULL;
    if (align <= _Alignof(max_align_t)) p = malloc(size);
    else if (posix_memalign(&p, align, size) != 0) p = NULL;
    if (p) ++live;
    return p;
}
void audit_free(void *p) { if (p) { --live; free(p); } }
void *audit_realloc(void *p, size_t old_size, size_t align, size_t size) {
    /* Always move: a failed replacement preserves the old allocation. */
    void *q = audit_alloc(size, align);
    if (q) { memcpy(q, p, old_size < size ? old_size : size); audit_free(p); }
    return q;
}
extern uint32_t audit_vec(size_t, size_t);
extern uint32_t audit_box(size_t, size_t);
extern uint32_t audit_string(size_t, size_t);
extern uint32_t audit_shared(size_t, size_t);
extern uint32_t audit_btree(size_t, size_t);
extern uint32_t audit_owned(size_t, size_t);
extern uint32_t audit_btree_set(size_t, size_t);
extern uint32_t audit_hash_map(size_t, size_t);
extern uint32_t audit_hash_set(size_t, size_t);
extern uint32_t audit_binary_heap(size_t, size_t);
extern uint32_t audit_deque(size_t, size_t);
extern uint32_t audit_scratch_slice(size_t, size_t);
extern size_t audit_negative(size_t);
int main(int argc, char **argv) {
    size_t seed = argc > 1 ? strtoull(argv[1], NULL, 10) : 17;
    volatile size_t control = audit_negative(seed);
    (void)control;
    size_t runs = 0, refusals = 0;
    for (unsigned kind = 0; kind < 12; ++kind) {
        uint32_t (*operation)(size_t, size_t) = kind == 11 ? audit_scratch_slice : kind == 10 ? audit_deque : kind == 9 ? audit_binary_heap : kind == 8 ? audit_hash_set : kind == 7 ? audit_hash_map : kind == 6 ? audit_btree_set : kind == 5 ? audit_owned : kind == 4 ? audit_btree : kind == 3 ? audit_shared : kind == 2 ? audit_string : kind ? audit_box : audit_vec;
        calls = fail_at = 0;
        uint32_t status = operation(seed, SIZE_MAX);
        if (status || live) return 10;
        size_t allocations = calls;
        ++runs;
        for (size_t failure = 1; failure <= allocations; ++failure) {
            calls = 0; fail_at = failure;
            status = operation(seed, SIZE_MAX);
            if (status != 1 || live) return 11;
            ++runs; ++refusals;
        }
        calls = fail_at = 0;
        status = operation(seed, 0);
        if (status != 1 || live) return 12;
        ++runs; ++refusals;
    }
    printf("{\"runs\":%zu,\"refusals\":%zu,\"live\":%zu}\n", runs, refusals, live);
    return 0;
}

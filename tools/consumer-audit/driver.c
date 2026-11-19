/* Test-only ELF root. libc is a trusted boundary, not a verified implementation. */
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
static size_t calls, fail_at, live;
void *audit_alloc(size_t size, size_t align) {
    if (++calls == fail_at) return NULL;
    void *ptr = NULL;
    if (align <= _Alignof(max_align_t)) ptr = malloc(size);
    else if (posix_memalign(&ptr, align, size) != 0) ptr = NULL;
    if (ptr) ++live;
    return ptr;
}
void audit_free(void *ptr) { if (ptr) { --live; free(ptr); } }
void *audit_realloc(void *ptr, size_t old_size, size_t align, size_t size) {
    void *next = audit_alloc(size, align);
    if (next) { memcpy(next, ptr, old_size < size ? old_size : size); audit_free(ptr); }
    return next;
}
extern uint32_t consumer_audit(size_t);
int main(void) {
    const size_t seeds[] = {0, 1, 2, 3, 7, 16, 31, 32, 63, 64, SIZE_MAX - 1, SIZE_MAX};
    size_t runs = 0;
    for (size_t i = 0; i < sizeof(seeds)/sizeof(seeds[0]); ++i) {
        calls = fail_at = 0;
        uint32_t result = consumer_audit(seeds[i]);
#ifdef AUDIT_REQUIRE_SUCCESS
        if (result != 0 || live) return 10;
#else
        if (result > 1 || live) return 10;
#endif
        size_t allocations = calls;
        ++runs;
        for (size_t failure = 1; failure <= allocations; ++failure) {
            calls = 0; fail_at = failure;
            result = consumer_audit(seeds[i]);
#ifdef AUDIT_REQUIRE_SUCCESS
            if (result != 1 || live) return 11;
#else
            if (result > 1 || live) return 11;
#endif
            ++runs;
        }
    }
    printf("%zu consumer runs; no live fixture allocations\n", runs);
    return 0;
}

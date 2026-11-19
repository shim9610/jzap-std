// Diagnostic only: locate copy call sites in the original PIE executable.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stddef.h>
#include <stdio.h>
#include <stdint.h>
static void *(*real_copy)(void *, const void *, size_t);
static int seen;
static __thread int tracing;
__attribute__((constructor)) static void init(void) {
    real_copy = dlsym(RTLD_NEXT, "memcpy");
}
void *memcpy(void *dst, const void *src, size_t n) {
    if (!real_copy) {
        volatile unsigned char *d = dst;
        const volatile unsigned char *s = src;
        for (size_t i=0;i<n;++i) d[i]=s[i];
        return dst;
    }
    if (n==1048576 && !tracing && seen<8) {
        tracing=1;
        void *caller=__builtin_return_address(0);
        Dl_info info;
        if (dladdr(caller,&info)) {
            fprintf(stderr,"copy=%d caller=0x%zx n=%zu src_page_offset=%zu dst_page_offset=%zu\n",
                    ++seen,(size_t)((uintptr_t)caller-(uintptr_t)info.dli_fbase),n,
                    (size_t)((uintptr_t)src%4096),(size_t)((uintptr_t)dst%4096));
        }
        tracing=0;
    }
    return real_copy(dst,src,n);
}

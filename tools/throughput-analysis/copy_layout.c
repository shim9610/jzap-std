#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#define BYTES (1u << 20)
typedef void *(*copier)(void *, const void *, size_t);
static double now(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) abort();
    return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
}
__attribute__((noinline))
static double trial(unsigned char *dst, const unsigned char *src,
                    unsigned char *tmp, uint64_t n, unsigned mode, copier copy) {
    double start = now();
    for (uint64_t i = 0; i < n; ++i) {
        size_t at = (size_t)(i & 4095u) * 256u;
        if (mode != 2) copy(tmp, src + at, 256);
        if (mode != 1) copy(dst + at, tmp, 256);
    }
    return now() - start;
}
int main(void) {
    unsigned char *src = aligned_alloc(64, BYTES + 64);
    unsigned char *dst = aligned_alloc(64, BYTES + 64);
    _Alignas(64) unsigned char storage[320];
    if (!src || !dst) return 2;
    for (size_t i = 0; i < BYTES + 64; ++i) src[i] = (unsigned char)(i * 17u + 3u);
    memset(dst, 0, BYTES + 64);
    puts("mode,phase,iterations,seconds,ns_per_block");
    for (unsigned mode = 0; mode != 3; ++mode) {
        uint64_t n = 1000000;
        memcpy(storage, src + 16, 256);
        double calibration = trial(dst + 16, src + 16, storage, n, mode, memcpy);
        n = (uint64_t)((double)n * 1.3 / calibration) + 1;
        for (;;) {
            double times[4], shortest = 1e100;
            for (unsigned j = 0; j != 4; ++j) {
                unsigned char *tmp = storage + j * 16;
                memcpy(tmp, src + 16, 256);
                times[j] = trial(dst + 16, src + 16, tmp, n, mode, memcpy);
                if (times[j] < shortest) shortest = times[j];
                if (memcmp(tmp, src + 16, 256)) return 3;
                if (mode != 1 && memcmp(dst + 16, src + 16, BYTES)) return 4;
            }
            if (shortest < 1.0) { n = (uint64_t)((double)n * 1.25 / shortest) + 1; continue; }
            for (unsigned j = 0; j != 4; ++j)
                printf("%u,%u,%llu,%.9f,%.6f\n",mode,j*16,
                       (unsigned long long)n,times[j],times[j]*1e9/(double)n);
            fflush(stdout);
            break;
        }
    }
    free(dst); free(src);
    return 0;
}

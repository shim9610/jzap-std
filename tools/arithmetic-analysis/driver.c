#include <stddef.h>
#include <stdint.h>
typedef struct { size_t value, failed; } Value;
extern Value audit_add(size_t, size_t);
extern Value audit_sub(size_t, size_t);
extern Value audit_mul(size_t, size_t);
extern Value audit_div(size_t, size_t);
extern Value audit_rem(size_t, size_t);
extern Value audit_wrap_rem(size_t, size_t);
extern Value audit_count_add(size_t, size_t);
extern Value audit_count_sub(size_t, size_t);
extern Value audit_count_mul(size_t, size_t);
extern Value audit_count_div(size_t, size_t);
extern Value audit_count_rem(size_t, size_t);
extern size_t audit_wrap_add(size_t, size_t);
extern size_t audit_wrap_sub(size_t, size_t);
extern size_t audit_wrap_mul(size_t, size_t);
extern Value audit_wrap_div(size_t, size_t);
extern Value audit_signed_div(intptr_t, intptr_t);
extern Value audit_signed_wrap_div(intptr_t, intptr_t);
int main(int argc, char **argv) {
    (void)argv;
#ifdef ARITHMETIC_NEGATIVE_CONTROL
    extern size_t audit_panic_control(size_t);
    return (int)audit_panic_control((size_t)argc);
#endif
    const size_t inputs[] = {0, 1, 2, 17, SIZE_MAX/2, SIZE_MAX/2+1, SIZE_MAX-1, SIZE_MAX};
    for (size_t i = 0; i < sizeof inputs / sizeof *inputs; i++) {
        for (size_t j = 0; j < sizeof inputs / sizeof *inputs; j++) {
            size_t expected;
            Value a = audit_add(inputs[i], inputs[j]);
            int failed = __builtin_add_overflow(inputs[i], inputs[j], &expected);
            if (a.failed != (size_t)failed || (!failed && a.value != expected)) return 1;
            a = audit_sub(inputs[i], inputs[j]);
            failed = __builtin_sub_overflow(inputs[i], inputs[j], &expected);
            if (a.failed != (size_t)failed || (!failed && a.value != expected)) return 2;
            a = audit_mul(inputs[i], inputs[j]);
            failed = __builtin_mul_overflow(inputs[i], inputs[j], &expected);
            if (a.failed != (size_t)failed || (!failed && a.value != expected)) return 3;
            if (audit_wrap_add(inputs[i], inputs[j]) != inputs[i] + inputs[j]) return 4;
            if (audit_wrap_sub(inputs[i], inputs[j]) != inputs[i] - inputs[j]) return 5;
            if (audit_wrap_mul(inputs[i], inputs[j]) != inputs[i] * inputs[j]) return 6;
            a = audit_div(inputs[i], inputs[j]);
            Value w = audit_wrap_div(inputs[i], inputs[j]);
            failed = inputs[j] == 0;
            if (a.failed != (size_t)(failed ? 2 : 0) || w.failed != a.failed) return 7;
            if (!failed && (a.value != inputs[i] / inputs[j] || w.value != a.value)) return 8;
            a = audit_rem(inputs[i], inputs[j]); w = audit_wrap_rem(inputs[i], inputs[j]);
            if (a.failed != (size_t)(failed ? 2 : 0) || w.failed != a.failed) return 12;
            if (!failed && (a.value != inputs[i] % inputs[j] || w.value != a.value)) return 13;
            Value typed = audit_count_add(inputs[i], inputs[j]);
            failed = __builtin_add_overflow(inputs[i], inputs[j], &expected);
            if (typed.failed != (size_t)failed || (!failed && typed.value != expected)) return 14;
            typed = audit_count_sub(inputs[i], inputs[j]);
            failed = __builtin_sub_overflow(inputs[i], inputs[j], &expected);
            if (typed.failed != (size_t)failed || (!failed && typed.value != expected)) return 15;
            typed = audit_count_mul(inputs[i], inputs[j]);
            failed = __builtin_mul_overflow(inputs[i], inputs[j], &expected);
            if (typed.failed != (size_t)failed || (!failed && typed.value != expected)) return 16;
            a = audit_div(inputs[i], inputs[j]); typed = audit_count_div(inputs[i], inputs[j]);
            if (typed.failed != a.failed || (!a.failed && typed.value != a.value)) return 17;
            a = audit_rem(inputs[i], inputs[j]); typed = audit_count_rem(inputs[i], inputs[j]);
            if (typed.failed != a.failed || (!a.failed && typed.value != a.value)) return 18;
        }
    }
    const intptr_t signed_inputs[] = {INTPTR_MIN, INTPTR_MIN+1, -17, -1, 0, 1, 17, INTPTR_MAX};
    for (size_t i = 0; i < sizeof signed_inputs / sizeof *signed_inputs; i++) {
        for (size_t j = 0; j < sizeof signed_inputs / sizeof *signed_inputs; j++) {
            intptr_t x = signed_inputs[i], y = signed_inputs[j];
            Value a = audit_signed_div(x, y), w = audit_signed_wrap_div(x, y);
            if (y == 0) {
                if (a.failed != 2 || w.failed != 2) return 9;
            } else if (x == INTPTR_MIN && y == -1) {
                if (a.failed != 1 || w.failed != 0 || w.value != (size_t)INTPTR_MIN) return 10;
            } else {
                if (a.failed || w.failed || a.value != (size_t)(x / y) || w.value != a.value) return 11;
            }
        }
    }
    return 0;
}

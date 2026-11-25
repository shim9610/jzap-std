/* V1 of ../cbmc-c/member_array.c alone: an array-typed pointer to the member at offset 1. */
#include <assert.h>
struct S { unsigned char a; unsigned char prefix[1]; };
unsigned char nondet_uchar(void);
unsigned long nondet_ulong(void);
int main(void) {
  unsigned char v[2] = { nondet_uchar(), nondet_uchar() };
  struct S keys[2] = { { 9, { v[0] } }, { 9, { v[1] } } };
  unsigned long r = nondet_ulong();
  __CPROVER_assume(r < 2);
  unsigned char (*p)[1] = &keys[r].prefix;
  assert((*p)[0] == v[r]);
  return 0;
}

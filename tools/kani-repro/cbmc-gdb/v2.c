/* V2 of ../cbmc-c/member_array.c alone: the decayed member array. */
#include <assert.h>
struct S { unsigned char a; unsigned char prefix[1]; };
unsigned char nondet_uchar(void);
unsigned long nondet_ulong(void);
int main(void) {
  unsigned char v[2] = { nondet_uchar(), nondet_uchar() };
  struct S keys[2] = { { 9, { v[0] } }, { 9, { v[1] } } };
  unsigned long r = nondet_ulong();
  __CPROVER_assume(r < 2);
  unsigned char *q = keys[r].prefix;
  assert(q[0] == v[r]);
  return 0;
}

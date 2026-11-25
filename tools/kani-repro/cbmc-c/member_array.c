/* CBMC-only reproduction of the fault behind reproductions (a)-(c).
 *
 * A pointer to an array-typed member at a nonzero offset of a struct that is
 * an element of an array selected by a symbolic index. Every read below is of
 * keys[r].prefix[0], which is v[r] by construction; native.c checks this for
 * every value. CBMC 6.11.0 reports V1 and V2 as failing. */
#include <assert.h>
struct S { unsigned char a; unsigned char prefix[1]; };
struct Z { unsigned char prefix[1]; unsigned char a; };
struct T { unsigned char a; unsigned char b; };
unsigned char nondet_uchar(void);
unsigned long nondet_ulong(void);
int main(void) {
  unsigned char v[2] = { nondet_uchar(), nondet_uchar() };
  struct S keys[2] = { { 9, { v[0] } }, { 9, { v[1] } } };
  struct Z zeros[2] = { { { v[0] }, 9 }, { { v[1] }, 9 } };
  struct T t[2] = { { 9, v[0] }, { 9, v[1] } };
  unsigned long r = nondet_ulong();
  __CPROVER_assume(r < 2);
  /* V1: array-typed pointer to the member at offset 1 */
  unsigned char (*p)[1] = &keys[r].prefix;
  assert((*p)[0] == v[r]);
  /* V2: element pointer from the decayed member array */
  unsigned char *q = keys[r].prefix;
  assert(q[0] == v[r]);
  /* V3 (control): the same element as a place expression, no pointer */
  assert(keys[r].prefix[0] == v[r]);
  /* V4 (control): the array member at offset 0 */
  unsigned char (*z)[1] = &zeros[r].prefix;
  assert((*z)[0] == v[r]);
  /* V5 (control): a pointer to a scalar member at offset 1 */
  unsigned char *b = &t[r].b;
  assert(*b == v[r]);
  return 0;
}

/* The assertions of member_array.c, natively, for every value of v and r. */
#include <assert.h>
#include <stdio.h>
struct S { unsigned char a; unsigned char prefix[1]; };
struct Z { unsigned char prefix[1]; unsigned char a; };
struct T { unsigned char a; unsigned char b; };
int main(void) {
  unsigned long cases = 0;
  for (int x = 0; x < 256; x++)
    for (int y = 0; y < 256; y++)
      for (unsigned long r = 0; r < 2; r++) {
        unsigned char v[2] = { (unsigned char)x, (unsigned char)y };
        struct S keys[2] = { { 9, { v[0] } }, { 9, { v[1] } } };
        struct Z zeros[2] = { { { v[0] }, 9 }, { { v[1] }, 9 } };
        struct T t[2] = { { 9, v[0] }, { 9, v[1] } };
        unsigned char (*p)[1] = &keys[r].prefix;
        unsigned char *q = keys[r].prefix;
        unsigned char (*z)[1] = &zeros[r].prefix;
        unsigned char *b = &t[r].b;
        assert((*p)[0] == v[r]);
        assert(q[0] == v[r]);
        assert(keys[r].prefix[0] == v[r]);
        assert((*z)[0] == v[r]);
        assert(*b == v[r]);
        cases++;
      }
  printf("native: %lu cases, every assertion holds\n", cases);
  return 0;
}

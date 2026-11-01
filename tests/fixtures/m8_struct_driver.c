#include <stdio.h>
#include <stddef.h>

struct M8Pair { int first; long second; };
extern struct M8Pair m8_global;
extern long m8_local(long);
extern long m8_array(long);
extern long m8_global_update(long);
extern long m8_copy_local(long);
extern long m8_copy_pointer(struct M8Pair *, struct M8Pair *);
struct M8WithArray { unsigned short values[3]; long marker; };
extern struct M8WithArray m8_array_member_global;
extern long m8_array_member_local(int);
extern long m8_array_member_global_update(int);

int main(void)
{
    /* GCC's sizeof/alignof/offsetof are part of the comparison oracle. */
    printf("%zu %zu %zu\n", sizeof(struct M8Pair),
           _Alignof(struct M8Pair), offsetof(struct M8Pair, second));
    printf("%ld %ld %ld\n", m8_local(7), m8_array(9),
           m8_global_update(13));
    printf("%d %ld\n", m8_global.first, m8_global.second);
    printf("%ld %ld %ld\n", m8_local(-8), m8_array(-3),
           m8_global_update(41));
    printf("%d %ld\n", m8_global.first, m8_global.second);
    struct M8Pair src = { -12, 105 }, dst = { 1, -1 };
    printf("%ld %ld\n", m8_copy_local(8), m8_copy_local(-6));
    long copied = m8_copy_pointer(&dst, &src);
    printf("%ld %d %ld\n", copied, dst.first, dst.second);
    printf("%zu %zu %zu\n", sizeof(struct M8WithArray),
           _Alignof(struct M8WithArray), offsetof(struct M8WithArray, marker));
    printf("%ld %ld\n", m8_array_member_local(15),
           m8_array_member_global_update(21));
    printf("%hu %hu %hu %ld\n", m8_array_member_global.values[0],
           m8_array_member_global.values[1],
           m8_array_member_global.values[2], m8_array_member_global.marker);
    return 0;
}

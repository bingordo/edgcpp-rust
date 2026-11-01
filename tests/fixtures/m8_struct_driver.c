#include <stdio.h>
#include <stddef.h>

struct M8Pair { int first; long second; };
extern struct M8Pair m8_global;
extern long m8_local(long);
extern long m8_array(long);
extern long m8_global_update(long);

int main(void)
{
    /* GCC's sizeof/alignof/offsetof are part of the comparison oracle. */
    printf("%zu %zu %zu\\n", sizeof(struct M8Pair),
           _Alignof(struct M8Pair), offsetof(struct M8Pair, second));
    printf("%ld %ld %ld\\n", m8_local(7), m8_array(9),
           m8_global_update(13));
    printf("%d %ld\\n", m8_global.first, m8_global.second);
    printf("%ld %ld %ld\\n", m8_local(-8), m8_array(-3),
           m8_global_update(41));
    printf("%d %ld\\n", m8_global.first, m8_global.second);
    return 0;
}

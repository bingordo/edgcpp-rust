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

struct M8Nested {
    struct M8Pair inner;
    unsigned short values[3];
    long tail;
};
extern struct M8Nested m8_nested_global;
extern long m8_nested_fields(long);
extern long m8_nested_copy(long);
extern long m8_nested_self_copy(struct M8Nested *);
extern long m8_nested_global_update(long);
extern long m8_array_field_copy(long);

union M8Union { int first; long second; unsigned short values[4]; };
extern union M8Union m8_union_global;
extern long m8_union_local(long);
extern long m8_union_copy(long);
extern long m8_union_pointer_copy(union M8Union *, union M8Union *);
extern long m8_union_self_copy(union M8Union *);
extern long m8_union_array(long);
extern long m8_union_global_update(long);
struct M8TaggedUnion { int tag; union M8Union data; };
extern struct M8TaggedUnion m8_tagged_union;
extern long m8_union_nested(long);

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
    printf("%zu %zu %zu %zu %zu\\n",
           sizeof(struct M8Nested), _Alignof(struct M8Nested),
           offsetof(struct M8Nested, inner),
           offsetof(struct M8Nested, values),
           offsetof(struct M8Nested, tail));
    printf("%ld %ld %ld %ld\\n",
           m8_nested_fields(7), m8_nested_fields(-3),
           m8_nested_copy(8), m8_nested_copy(1));
    printf("%ld %ld %ld\\n", m8_nested_global_update(13),
           m8_array_field_copy(9), m8_array_field_copy(27));
    printf("%d %ld %hu %hu %hu %ld\\n",
           m8_nested_global.inner.first, m8_nested_global.inner.second,
           m8_nested_global.values[0], m8_nested_global.values[1],
           m8_nested_global.values[2], m8_nested_global.tail);
    struct M8Nested value = {{3,17}, {4,9,10}, 21};
    printf("%ld %ld %hu\\n", m8_nested_self_copy(&value),
           value.inner.second, value.values[1]);
    printf("%zu %zu %zu %zu\\n",
           sizeof(union M8Union), _Alignof(union M8Union),
           offsetof(union M8Union, first), offsetof(union M8Union, second));
    printf("%ld %ld %ld %ld\\n",
           m8_union_local(5), m8_union_local(-4),
           m8_union_copy(17), m8_union_array(12));
    union M8Union src, dst;
    src.second = 127;
    dst.first = -1;
    printf("%ld %ld %ld\\n",
           m8_union_pointer_copy(&dst, &src),
           m8_union_self_copy(&src), dst.second);
    printf("%ld %ld %ld %ld\\n",
           m8_union_global_update(11), m8_union_global.second,
           m8_union_nested(13), m8_tagged_union.data.second);
    printf("%zu %zu %zu\\n",
           sizeof(struct M8TaggedUnion),
           _Alignof(struct M8TaggedUnion),
           offsetof(struct M8TaggedUnion, data));
    return 0;
}

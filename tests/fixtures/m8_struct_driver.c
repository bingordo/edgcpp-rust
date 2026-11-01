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

extern long m8_initialized_local_pair(void);
extern long m8_initialized_partial_pair(void);
extern long m8_initialized_designated_pair(void);
extern long m8_initialized_local_array(void);
extern long m8_initialized_nested(void);
extern long m8_initialized_union(void);
extern long m8_initialized_global_sum(void);
extern struct M8Pair m8_static_pair;
extern unsigned short m8_static_vector[5];
extern struct M8WithArray m8_static_nested;
extern union M8Union m8_static_union;

extern long m8_runtime_braced(long);
extern long m8_dynamic_array(long);
extern long m8_dynamic_struct(long);
extern long m8_dynamic_nested(long);
extern long m8_dynamic_union(long);
extern long m8_dynamic_once_array(long);
extern long m8_dynamic_once_struct(long);
extern long m8_dynamic_once_nested(long);
extern long m8_dynamic_once_union(long);
extern long m8_dynamic_multidimensional(long);
extern long m8_static_pointer_relocs(void);
struct M8PointerHolder { int tag; int *pointer; };
extern struct M8PointerHolder m8_pointer_to_forward;
extern struct M8PointerHolder m8_pointer_to_element;
extern int m8_reloc_target;

extern long m8_bitfields_local(long);
extern long m8_bitfields_global(long);
extern long m8_bitfields_array(long);
extern long m8_bitfields_copy(long);
extern long m8_bitunion(long);
struct M8BitFields {
    unsigned int first : 3;
    unsigned int second : 5;
    signed int negative : 6;
    unsigned int fourth : 9;
    unsigned int fifth : 5;
    unsigned char sentinel;
};
extern struct M8BitFields m8_bits_global;
extern struct M8BitFields m8_bits_array[2];

extern long m8_bitfield_compound(long);
extern long m8_bitfield_compound_global(long);
struct M8CompoundBits {
    unsigned int a : 4;
    unsigned int b : 5;
    signed int c : 6;
    unsigned int d : 9;
};
extern struct M8CompoundBits m8_compound_global;

extern long m8_bitfields_constant_local(void);
extern long m8_bitfields_partial_local(void);
extern long m8_bitfields_constant_global(void);
extern struct M8BitFields m8_bits_constant;
extern struct M8BitFields m8_bits_partial;
union M8BitUnion { unsigned int active : 7; unsigned int raw; };
extern union M8BitUnion m8_bitunion_constant;

struct M8AbiPair { int first; int second; };
struct M8AbiMixed { unsigned char tag; int amount; };
struct M8AbiTiny { unsigned char value; };
struct M8AbiShort { short first; short second; };
extern long m8_abi_take_pair(struct M8AbiPair);
extern long m8_abi_take_mixed(long, struct M8AbiMixed, int);
extern long m8_abi_take_tiny(struct M8AbiTiny, long);
extern long m8_abi_take_short(struct M8AbiShort);
extern struct M8AbiPair m8_abi_return_pair(int,int);
extern struct M8AbiMixed m8_abi_return_mixed(unsigned char,int);
extern struct M8AbiTiny m8_abi_return_tiny(unsigned char);
extern struct M8AbiPair m8_abi_roundtrip_pair(struct M8AbiPair);

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
    union M8Union union_src, union_dst;
    union_src.second = 127;
    union_dst.first = -1;
    printf("%ld %ld %ld\\n",
           m8_union_pointer_copy(&union_dst, &union_src),
           m8_union_self_copy(&union_src), union_dst.second);
    printf("%ld %ld %ld %ld\\n",
           m8_union_global_update(11), m8_union_global.second,
           m8_union_nested(13), m8_tagged_union.data.second);
    printf("%zu %zu %zu\\n",
           sizeof(struct M8TaggedUnion),
           _Alignof(struct M8TaggedUnion),
           offsetof(struct M8TaggedUnion, data));
    printf("%ld %ld %ld %ld %ld %ld %ld\\n",
           m8_initialized_local_pair(), m8_initialized_partial_pair(),
           m8_initialized_designated_pair(), m8_initialized_local_array(),
           m8_initialized_nested(), m8_initialized_union(),
           m8_initialized_global_sum());
    printf("%d %ld %hu %hu %hu %hu %ld %ld\\n",
           m8_static_pair.first, m8_static_pair.second,
           m8_static_vector[0], m8_static_vector[3],
           m8_static_nested.values[0], m8_static_nested.values[2],
           m8_static_nested.marker, m8_static_union.second);
    printf("%ld %ld %ld\\n", m8_runtime_braced(11), m8_runtime_braced(-7), m8_runtime_braced(0));
    printf("%ld %ld %ld %ld\\n",
           m8_dynamic_array(7), m8_dynamic_struct(-9),
           m8_dynamic_nested(4), m8_dynamic_union(11));
    printf("%ld %ld %ld %ld\\n",
           m8_dynamic_array(-2), m8_dynamic_struct(21),
           m8_dynamic_nested(0), m8_dynamic_union(-3));
    printf("%ld %ld %ld %ld %ld\\n",
           m8_dynamic_once_array(7), m8_dynamic_once_struct(-3),
           m8_dynamic_once_nested(11), m8_dynamic_once_union(2),
           m8_dynamic_multidimensional(13));
    printf("%ld %ld %ld %ld %ld\\n",
           m8_dynamic_once_array(-8), m8_dynamic_once_struct(21),
           m8_dynamic_once_nested(0), m8_dynamic_once_union(-17),
           m8_dynamic_multidimensional(-5));
    printf("%ld %zu %zu %d %d %d\\n",
           m8_static_pointer_relocs(),
           sizeof(struct M8PointerHolder),
           offsetof(struct M8PointerHolder, pointer),
           *m8_pointer_to_forward.pointer,
           *m8_pointer_to_element.pointer,
           m8_reloc_target);
    printf("%zu %zu %ld %ld %ld %ld %ld\\n",
           sizeof(struct M8BitFields), _Alignof(struct M8BitFields),
           m8_bitfields_local(77), m8_bitfields_local(-12),
           m8_bitfields_global(31), m8_bitfields_array(75),
           m8_bitfields_copy(17));
    printf("%u %u %d %u %u %u %ld %ld\\n",
           m8_bits_global.first, m8_bits_global.second,
           m8_bits_global.negative, m8_bits_global.fourth,
           m8_bits_global.fifth, m8_bits_global.sentinel,
           m8_bitunion(47), m8_bitunion(3));
    printf("%ld %ld %ld\\n",
           m8_bitfields_global(-9), m8_bitfields_copy(-3),
           m8_bitfields_array(22));
    printf("%ld %ld %ld %ld\\n",
           m8_bitfield_compound(23), m8_bitfield_compound(-17),
           m8_bitfield_compound_global(52),
           m8_bitfield_compound_global(-9));
    printf("%u %u %d %u\\n",
           m8_compound_global.a, m8_compound_global.b,
           m8_compound_global.c, m8_compound_global.d);
    printf("%ld %ld %ld %zu\\n",
           m8_bitfields_constant_local(), m8_bitfields_partial_local(),
           m8_bitfields_constant_global(), sizeof(struct M8BitFields));
    printf("%u %u %d %u %u %u %u\\n",
           m8_bits_constant.first, m8_bits_constant.second,
           m8_bits_constant.negative, m8_bits_constant.fourth,
           m8_bits_constant.fifth, m8_bits_constant.sentinel,
           m8_bitunion_constant.active);
    printf("%u %u %d %u %u %u\\n",
           m8_bits_partial.first, m8_bits_partial.second,
           m8_bits_partial.negative, m8_bits_partial.fourth,
           m8_bits_partial.fifth, m8_bits_partial.sentinel);
    struct M8AbiPair small_pair = { -23, 187 };
    struct M8AbiMixed small_mixed = { 143, -1496 };
    struct M8AbiTiny small_tiny = { 237 };
    struct M8AbiShort small_shorts = { -78, 115 };
    struct M8AbiPair returned_pair = m8_abi_return_pair(19, -31);
    struct M8AbiMixed returned_mixed = m8_abi_return_mixed(109, -723);
    struct M8AbiTiny returned_tiny = m8_abi_return_tiny(211);
    struct M8AbiPair roundtrip = m8_abi_roundtrip_pair(small_pair);
    printf("%zu %zu %zu %zu %ld %ld %ld %ld\\n",
           sizeof(struct M8AbiPair), sizeof(struct M8AbiMixed),
           sizeof(struct M8AbiTiny), sizeof(struct M8AbiShort),
           m8_abi_take_pair(small_pair),
           m8_abi_take_mixed(179, small_mixed, -7),
           m8_abi_take_tiny(small_tiny, -819),
           m8_abi_take_short(small_shorts));
    printf("%d %d %hhu %d %hhu %d %d\\n",
           returned_pair.first, returned_pair.second,
           returned_mixed.tag, returned_mixed.amount,
           returned_tiny.value,
           roundtrip.first, roundtrip.second);
    return 0;
}

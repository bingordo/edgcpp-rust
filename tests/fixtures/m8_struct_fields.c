/* M8: C struct storage, scalar field lvalues, dot/arrow and array stride.
 * Keep this fixture strictly within the supported integer/pointer ABI. */
struct M8Pair { int first; long second; };
struct M8Pair m8_global;

long m8_local(long n)
{
    struct M8Pair value;
    value.first = (int)n;
    value.second = n + 11;
    struct M8Pair *ptr = &value;
    ptr->second += ptr->first;
    return value.second;
}

long m8_array(long n)
{
    struct M8Pair slots[2];
    slots[0].first = (int)n;
    slots[0].second = n + 1;
    slots[1].first = slots[0].first + 2;
    slots[1].second = slots[0].second + 3;
    return slots[1].second + slots[1].first;
}

long m8_global_update(long n)
{
    m8_global.first = (int)n;
    m8_global.second = n + 10;
    return m8_global.first + m8_global.second;
}

/* C struct assignments copy values, including when accessed by pointers. */
long m8_copy_local(long n)
{
    struct M8Pair original, duplicate;
    original.first = (int)n;
    original.second = n + 25;
    duplicate = original;
    original.first = 0;
    original.second = 0;
    return duplicate.second + duplicate.first;
}

long m8_copy_pointer(struct M8Pair *dst, struct M8Pair *src)
{
    *dst = *src;
    return dst->second - dst->first;
}

/* Native array-member offset and bounded element strides. */
struct M8WithArray { unsigned short values[3]; long marker; };
struct M8WithArray m8_array_member_global;
long m8_array_member_local(int n)
{
    struct M8WithArray obj;
    obj.values[0] = (unsigned short)n;
    obj.values[1] = (unsigned short)(obj.values[0] + 3);
    obj.values[2] = (unsigned short)(obj.values[1] + 7);
    obj.marker = (long)obj.values[2] + 9;
    return obj.marker + obj.values[0];
}
long m8_array_member_global_update(int n)
{
    m8_array_member_global.values[0] = (unsigned short)n;
    m8_array_member_global.values[1] = (unsigned short)(n + 1);
    m8_array_member_global.values[2] = (unsigned short)(n + 2);
    m8_array_member_global.marker = (long)m8_array_member_global.values[1] * 5;
    return m8_array_member_global.marker + m8_array_member_global.values[2];
}

/* M8: recursive record validation and value-semantic scalar copy of nested
 * structs, including array members and intentional trailing ABI padding. */
struct M8Nested {
    struct M8Pair inner;
    unsigned short values[3];
    long tail;
};
struct M8Nested m8_nested_global;

long m8_nested_fields(long n)
{
    struct M8Nested node;
    node.inner.first = (int)n;
    node.inner.second = n + 19;
    node.values[0] = (unsigned short)(n + 1);
    node.values[1] = (unsigned short)(n + 2);
    node.values[2] = (unsigned short)(n + 3);
    node.tail = n * 4;
    struct M8Nested *ptr = &node;
    ptr->inner.second += ptr->values[1];
    return node.inner.second + node.inner.first + ptr->tail;
}

long m8_nested_copy(long n)
{
    struct M8Nested src, dst;
    src.inner.first = (int)n;
    src.inner.second = n + 21;
    src.values[0] = (unsigned short)(n + 1);
    src.values[1] = (unsigned short)(n + 2);
    src.values[2] = (unsigned short)(n + 3);
    src.tail = n * 3;
    dst = src;
    src.inner.first = 0;
    src.inner.second = 0;
    src.values[1] = 0;
    src.tail = 0;
    return dst.inner.first + dst.inner.second + dst.values[0] +
           dst.values[1] + dst.values[2] + dst.tail;
}

long m8_nested_self_copy(struct M8Nested *value)
{
    *value = *value;
    return value->inner.first + value->inner.second +
           value->values[0] + value->values[1] + value->values[2] +
           value->tail;
}

long m8_nested_global_update(long n)
{
    m8_nested_global.inner.first = (int)n;
    m8_nested_global.inner.second = n + 6;
    m8_nested_global.values[0] = (unsigned short)(n + 3);
    m8_nested_global.values[1] = (unsigned short)(n + 4);
    m8_nested_global.values[2] = (unsigned short)(n + 5);
    m8_nested_global.tail = n * 2;
    return m8_nested_global.inner.second + m8_nested_global.values[2] +
           m8_nested_global.tail;
}

long m8_array_field_copy(long n)
{
    struct M8WithArray src, dst;
    src.values[0] = (unsigned short)n;
    src.values[1] = (unsigned short)(n + 2);
    src.values[2] = (unsigned short)(n + 5);
    src.marker = n + 100;
    dst = src;
    src.values[0] = 0;
    src.marker = 0;
    return dst.values[0] + dst.values[1] + dst.values[2] + dst.marker;
}

/* M8: simple C unions use overlapping native offsets and have a single
 * active typed member at a time. Copies use bytes, never inactive members. */
union M8Union {
    int first;
    long second;
    unsigned short values[4];
};
union M8Union m8_union_global;

long m8_union_local(long n)
{
    union M8Union value;
    value.second = n + 5;
    union M8Union *ptr = &value;
    ptr->second += n;
    return value.second;
}

long m8_union_copy(long n)
{
    union M8Union src, dst;
    src.second = n + 25;
    dst = src;
    src.second = 0;
    return dst.second;
}

long m8_union_pointer_copy(union M8Union *dst, union M8Union *src)
{
    *dst = *src;
    return dst->second;
}

long m8_union_self_copy(union M8Union *p)
{
    *p = *p;
    return p->second;
}

long m8_union_array(long n)
{
    union M8Union slots[2];
    slots[0].first = (int)n;
    slots[1].second = n + 8;
    return slots[0].first + slots[1].second;
}

long m8_union_global_update(long n)
{
    m8_union_global.second = n * 3;
    return m8_union_global.second;
}

struct M8TaggedUnion { int tag; union M8Union data; };
struct M8TaggedUnion m8_tagged_union;
long m8_union_nested(long n)
{
    m8_tagged_union.tag = 7;
    m8_tagged_union.data.second = n + 9;
    return m8_tagged_union.data.second + m8_tagged_union.tag;
}

/* Constant C11 aggregate initialization (including partial and designated).
 * EDG supplies the subobject type/identity; Cranelift must preserve values,
 * padding, zero fill, order and native record/union layout. */
struct M8Pair m8_static_pair = { .second = 127, .first = -11 };
unsigned short m8_static_vector[5] = { [3] = 31, [0] = 7 };
struct M8WithArray m8_static_nested = {
    .marker = 91, .values = { [2] = 19, [0] = 4 }
};
union M8Union m8_static_union = { .second = 103 };

long m8_initialized_local_pair(void)
{
    struct M8Pair p = { -5, 37 };
    return p.first + p.second;
}
long m8_initialized_partial_pair(void)
{
    struct M8Pair p = { 11 };
    return p.first + p.second;
}
long m8_initialized_designated_pair(void)
{
    struct M8Pair p = { .second = 49, .first = 6 };
    return p.first + p.second;
}
long m8_initialized_local_array(void)
{
    unsigned short a[5] = { [4] = 13, [1] = 8 };
    return a[0] + a[1] + a[2] + a[3] + a[4];
}
long m8_initialized_nested(void)
{
    struct M8Nested p = {
        .inner = { .second = 45, .first = 2 },
        .values = { [2] = 3, [0] = 7 },
        .tail = 12
    };
    return p.inner.first + p.inner.second + p.values[0] +
           p.values[1] + p.values[2] + p.tail;
}
long m8_initialized_union(void)
{
    union M8Union u = { .second = 33 };
    return u.second;
}
long m8_initialized_global_sum(void)
{
    return m8_static_pair.first + m8_static_pair.second +
           m8_static_vector[0] + m8_static_vector[3] +
           m8_static_nested.values[0] + m8_static_nested.values[2] +
           m8_static_nested.marker + m8_static_union.second;
}

/* Nonconstant braced initialization is accepted only if EDG has already
 * lowered it into ordinary executable initialization operations. */
struct M8RuntimePair { int first; long second; };
long m8_runtime_braced(long input)
{
    struct M8RuntimePair value = { (int)input, input + 9 };
    return value.first + value.second;
}

/* Nonconstant C aggregate initializers: side effects and element order must
 * match GCC, not a precomputed constant image. */
long m8_dynamic_array(long n)
{
    long values[4] = { [0] = n, [2] = n + 3, [3] = n * 2 };
    return values[0] + values[1] + values[2] + values[3];
}
long m8_dynamic_struct(long n)
{
    struct M8Pair value = { .second = n + 3, .first = (int)(n - 2) };
    return value.first + value.second;
}
long m8_dynamic_nested(long n)
{
    struct M8Nested value = {
        .inner = { .first = (int)n, .second = n + 1 },
        .values = { [2] = (unsigned short)(n + 4) },
        .tail = n * 5
    };
    return value.inner.first + value.inner.second +
           value.values[0] + value.values[1] + value.values[2] +
           value.tail;
}
long m8_dynamic_union(long n)
{
    union M8Union u = { .second = n * 7 };
    return u.second;
}

/* M8 initializer execution: dynamic values must evaluate exactly once,
 * omitted members must be zero, and nested designated paths must survive
 * the EDG-to-Cranelift lowering. Use one side-effectful expression per
 * initializer so C's unspecified element evaluation order is irrelevant. */
static long m8_initializer_call_count;
static long m8_initializer_value(long n)
{
    m8_initializer_call_count += 1;
    return n + 4 * m8_initializer_call_count;
}

long m8_dynamic_once_array(long n)
{
    m8_initializer_call_count = 0;
    long values[4] = { [2] = m8_initializer_value(n) };
    return values[0] + values[1] + values[2] + values[3] +
           100 * m8_initializer_call_count;
}

long m8_dynamic_once_struct(long n)
{
    m8_initializer_call_count = 0;
    struct M8Pair value = { .second = m8_initializer_value(n) };
    return value.first + value.second + 100 * m8_initializer_call_count;
}

long m8_dynamic_once_nested(long n)
{
    m8_initializer_call_count = 0;
    struct M8Nested value = {
        .inner = { .second = m8_initializer_value(n) },
        .values = { [2] = (unsigned short)(n + 3) },
        .tail = 13
    };
    return value.inner.first + value.inner.second +
           value.values[0] + value.values[1] + value.values[2] +
           value.tail + 100 * m8_initializer_call_count;
}

long m8_dynamic_once_union(long n)
{
    m8_initializer_call_count = 0;
    union M8Union value = { .second = m8_initializer_value(n) };
    return value.second + 100 * m8_initializer_call_count;
}

long m8_dynamic_multidimensional(long n)
{
    long items[2][3] = { [1] = { [2] = n + 9, [0] = n - 2 } };
    return items[0][0] + items[0][1] + items[0][2] +
           items[1][0] + items[1][1] + items[1][2];
}

/* M8: static aggregate pointer fields require real ELF data relocations.
 * Exercise forward references, pointer addends and two relocations within
 * the same object. The referenced data is owned by the generated TU. */
struct M8PointerHolder { int tag; int *pointer; };
extern int m8_reloc_target;
struct M8PointerHolder m8_pointer_to_forward = { .tag = 5, .pointer = &m8_reloc_target };
int m8_reloc_target = 37;
static int m8_reloc_values[3] = { 3, 7, 11 };
struct M8PointerHolder m8_pointer_to_element = {
    .tag = 9, .pointer = &m8_reloc_values[1]
};
struct M8PointerArray { int padding; int *refs[2]; };
struct M8PointerArray m8_pointer_array = {
    .padding = 6, .refs = { &m8_reloc_target, &m8_reloc_values[2] }
};
long m8_static_pointer_relocs(void)
{
    return m8_pointer_to_forward.tag + *m8_pointer_to_forward.pointer +
           m8_pointer_to_element.tag + *m8_pointer_to_element.pointer +
           m8_pointer_array.padding + *m8_pointer_array.refs[0] +
           *m8_pointer_array.refs[1];
}

/* M8 native bitfield offsets, cross-byte field ranges and RMW semantics.
 * Assign values within signed range so the oracle is independent of
 * implementation-defined signed-overflow truncation. */
struct M8BitFields {
    unsigned int first : 3;
    unsigned int second : 5;
    signed int negative : 6;
    unsigned int fourth : 9;
    unsigned int fifth : 5;
    unsigned char sentinel;
};
struct M8BitFields m8_bits_global;
struct M8BitFields m8_bits_array[2];

long m8_bitfields_local(long n)
{
    struct M8BitFields obj;
    obj.first = 5;
    obj.second = 27;
    obj.negative = -13;
    obj.fourth = (unsigned int)n & 255u;
    obj.fifth = 21;
    obj.sentinel = 77;
    return obj.first + obj.second * 2 + obj.negative * 3 +
           obj.fourth + obj.fifth * 4 + obj.sentinel * 5;
}

long m8_bitfields_global(long n)
{
    m8_bits_global.first = 3;
    m8_bits_global.second = 19;
    m8_bits_global.negative = -9;
    m8_bits_global.fourth = (unsigned int)n & 255u;
    m8_bits_global.fifth = 13;
    m8_bits_global.sentinel = 101;
    return m8_bits_global.first + m8_bits_global.second +
           m8_bits_global.negative + m8_bits_global.fourth +
           m8_bits_global.fifth + m8_bits_global.sentinel;
}

long m8_bitfields_array(long n)
{
    m8_bits_array[0].first = 6;
    m8_bits_array[0].second = 12;
    m8_bits_array[1].negative = -6;
    m8_bits_array[1].fourth = (unsigned int)n & 127u;
    m8_bits_array[1].fifth = 11;
    return m8_bits_array[0].first + m8_bits_array[0].second +
           m8_bits_array[1].negative + m8_bits_array[1].fourth +
           m8_bits_array[1].fifth;
}

long m8_bitfields_copy(long n)
{
    struct M8BitFields src, dst;
    src.first = 2;
    src.second = 18;
    src.negative = -7;
    src.fourth = (unsigned int)n & 127u;
    src.fifth = 9;
    src.sentinel = 59;
    dst = src;
    src.first = 0;
    src.second = 0;
    src.negative = 0;
    src.fourth = 0;
    src.fifth = 0;
    src.sentinel = 0;
    return dst.first + dst.second + dst.negative +
           dst.fourth + dst.fifth + dst.sentinel;
}

union M8BitUnion {
    unsigned int active : 7;
    unsigned int raw;
};
long m8_bitunion(long n)
{
    union M8BitUnion value;
    value.active = (unsigned int)n & 63u;
    return value.active;
}

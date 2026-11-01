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

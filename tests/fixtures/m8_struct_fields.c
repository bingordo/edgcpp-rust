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

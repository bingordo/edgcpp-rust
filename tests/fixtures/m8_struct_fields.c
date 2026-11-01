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

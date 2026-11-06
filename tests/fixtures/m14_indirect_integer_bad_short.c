/* Narrow C integer return ABI classes need separate promotion rules. */
short m14_bad_indirect_short(short (*fn)(double),double x) {
    return fn(x);
}

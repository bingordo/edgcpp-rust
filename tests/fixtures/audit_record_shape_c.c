/* C11 native record-field completeness proof and GCC execution oracle. */
struct AuditTriple { int first; long second; unsigned char third; };
union AuditPair { int integer; long wider; };
static long sum(struct AuditTriple value) {
    return (long)value.first + value.second + value.third;
}
int main(void) {
    struct AuditTriple value = {1, 2, 3};
    union AuditPair variant = {.integer = 4};
    return (sum(value) == 6 && variant.integer == 4) ? 0 : 1;
}

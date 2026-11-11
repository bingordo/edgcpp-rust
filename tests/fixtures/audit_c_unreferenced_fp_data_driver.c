#include <stdint.h>
#include <stdio.h>
#include <string.h>

extern float audit_c_fp_negative_zero;
extern double audit_c_fp_two_point_five;
extern float audit_c_fp_tentative;
extern double audit_c_fp_extern_initialized;
extern int audit_c_fp_integer_only(void);

int main(void) {
    uint32_t negative_zero, zero;
    uint64_t two_point_five, minus_three_point_five;
    memcpy(&negative_zero, &audit_c_fp_negative_zero, sizeof negative_zero);
    memcpy(&zero, &audit_c_fp_tentative, sizeof zero);
    memcpy(&two_point_five, &audit_c_fp_two_point_five, sizeof two_point_five);
    memcpy(&minus_three_point_five, &audit_c_fp_extern_initialized,
           sizeof minus_three_point_five);
    printf("%08x %016llx %08x %016llx %d\n",
           (unsigned)negative_zero, (unsigned long long)two_point_five,
           (unsigned)zero, (unsigned long long)minus_three_point_five,
           audit_c_fp_integer_only());
    return 0;
}

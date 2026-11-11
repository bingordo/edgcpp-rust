#include <stdint.h>
#include <stdio.h>
#include <string.h>

extern int audit_data_only_integer;
extern float audit_data_only_negative_zero;
extern int audit_data_only_tentative;
struct AuditDataOnlyPair {int number; int *pointer;};
extern struct AuditDataOnlyPair audit_data_only_pair;

int main(void) {
    uint32_t fp_bits = 0;
    memcpy(&fp_bits, &audit_data_only_negative_zero, sizeof fp_bits);
    printf("%d %d %d %08x %d\n",
           audit_data_only_integer, audit_data_only_pair.number,
           audit_data_only_tentative, (unsigned)fp_bits,
           audit_data_only_pair.pointer == &audit_data_only_integer);
    return 0;
}

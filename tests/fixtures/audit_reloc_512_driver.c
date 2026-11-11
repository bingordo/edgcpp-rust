#include <stdio.h>
extern int audit_reloc_target;
extern int *audit_many_relocs[512];
extern int audit_reloc_touch(void);
int main(void) {
    long sum = 0;
    for (int i = 0; i < 512; ++i) {
        if (audit_many_relocs[i] != &audit_reloc_target) return 3;
        sum += *audit_many_relocs[i];
    }
    printf("%ld %d %d\n", sum, *audit_many_relocs[511], audit_reloc_touch());
    return 0;
}

#include <stdio.h>
extern int audit_routine_zero(void);
extern int audit_routine_indirect(int (*fn)(void));
int main(void) {
    if (audit_routine_zero() != 41 ||
        audit_routine_indirect(audit_routine_zero) != 42) {
        fputs("native C routine signature result mismatch\n", stderr);
        return 1;
    }
    puts("native C zero-parameter signature pass");
    return 0;
}

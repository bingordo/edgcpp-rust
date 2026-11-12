#include <stdio.h>
extern int audit_integer_pointer_call(void);
extern int *audit_integer_pointer_return(void);
extern int audit_integer_pointer_assign(void);
int audit_integer_pointer_sink(int *p) { return p == 0 ? 53 : -53; }
int main(void) {
    if (audit_integer_pointer_call() != 53 ||
        audit_integer_pointer_return() != 0 ||
        audit_integer_pointer_assign() != 1) {
        fputs("integer null pointer differential mismatch\n",stderr);
        return 1;
    }
    puts("C11 integer null-pointer conversions pass");
    return 0;
}

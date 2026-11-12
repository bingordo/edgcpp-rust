#include <stdio.h>
#include <stdlib.h>

int audit_cv_sink_const(const int *p) { return *p + 11; }
int audit_cv_sink_void(const void *p) { return p != NULL ? 19 : -19; }
int audit_cv_sink_volatile(const volatile int *p) { return *p * 3; }

extern int audit_cv_from_mutable(int *);
extern int audit_cv_from_const(const int *);
extern int audit_cv_array_decay(int [4]);
extern int audit_cv_to_const_void(const int *);
extern int audit_cv_add_volatile(int *);

int main(void)
{
    int value = 7;
    const int read_only = 23;
    int values[4] = {13, 0, 0, 0};
    int actual[] = {
        audit_cv_from_mutable(&value),
        audit_cv_from_const(&read_only),
        audit_cv_array_decay(values),
        audit_cv_to_const_void(&read_only),
        audit_cv_add_volatile(&value)
    };
    const int expected[] = {18, 34, 24, 19, 21};
    for (unsigned i = 0; i < sizeof(actual) / sizeof(actual[0]); i++) {
        if (actual[i] != expected[i]) {
            fprintf(stderr, "CV call mismatch at %u: got %d expected %d\n",
                    i, actual[i], expected[i]);
            return 1;
        }
    }
    puts("audit C11 CV pointer calls pass");
    return 0;
}

#include <stdio.h>

extern int audit_cv_context_assign(int *);
extern const int *audit_cv_context_return(int *);
extern int audit_cv_context_const_void(const int *);
extern const int **audit_cv_context_explicit(int **);

int audit_cv_context_sink(const int *p) { return *p + 17; }

int main(void)
{
    int x = 31;
    int *ptr = &x;
    if (audit_cv_context_assign(&x) != 48 ||
        audit_cv_context_return(&x) != &x ||
        audit_cv_context_const_void(&x) != 1 ||
        (const void *)audit_cv_context_explicit(&ptr) != (const void *)&ptr) {
        fputs("audit CV pointer assignment or return mismatch\n",stderr);
        return 1;
    }
    puts("audit C11 CV assignment/return pass");
    return 0;
}

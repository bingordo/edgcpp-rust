#include <stdio.h>

extern int audit_c_initialized_export;
extern int audit_c_tentative_export;
extern int audit_c_extern_initialized_export;
extern int audit_c_unrelated_function(void);

int main(void) {
    printf("%d %d %d %d\n",
           audit_c_initialized_export,
           audit_c_tentative_export,
           audit_c_extern_initialized_export,
           audit_c_unrelated_function());
    return 0;
}

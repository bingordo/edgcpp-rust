#include <stdio.h>
extern int audit_pointer_cmp_qualified(int *,const int *);
extern int audit_pointer_cmp_void(int *,const void *);
extern long audit_pointer_diff_same_array(int *,int *);

int main(void)
{
    int values[4] = {3,5,7,11};
    int independent = 17;
    if (audit_pointer_cmp_qualified(&values[0],&values[0]) != 1 ||
        audit_pointer_cmp_qualified(&values[0],&independent) != 0 ||
        audit_pointer_cmp_void(&values[0],&values[0]) != 1 ||
        audit_pointer_diff_same_array(&values[0],&values[3]) != 3) {
        fputs("C11 pointer compare or difference mismatch\n",stderr);
        return 1;
    }
    puts("C11 pointer comparison/difference pass");
    return 0;
}

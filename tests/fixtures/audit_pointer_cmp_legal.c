/* ISO C11 pointer equality and same-array pointer subtraction. */
int audit_pointer_cmp_qualified(int *p, const int *q)
{
    return p == q;
}

int audit_pointer_cmp_void(int *p, const void *q)
{
    return p == q;
}

long audit_pointer_diff_same_array(int *first, int *last)
{
    return last - first;
}

/* Constraint violation: pointer subtraction needs compatible object pointers. */
long audit_pointer_cmp_bad_diff(int *a, long *b)
{
    return a - b;
}

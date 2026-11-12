/* Constraint violation: unequal unrelated object pointer types. */
int audit_pointer_cmp_bad_eq(int *a, long *b)
{
    return a == b;
}

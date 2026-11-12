/* Constraint violation: relational pointer comparison needs compatible pointees. */
int audit_pointer_cmp_bad_order(int *a, long *b)
{
    return a < b;
}

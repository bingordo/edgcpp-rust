/* C11 constraint violation: relational ordering between void* and object*. */
int audit_pointer_cmp_bad_void(void *a, int *b)
{
    return a < b;
}

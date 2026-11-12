/* ISO C11 does not define relational ordering of void pointers. */
int audit_pointer_cmp_bad_void(void *a, void *b)
{
    return a < b;
}

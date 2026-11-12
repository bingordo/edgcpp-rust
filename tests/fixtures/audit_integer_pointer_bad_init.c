/* C11 constraint violation: nonzero integer initializes pointer. */
int audit_integer_pointer_bad_init(void) {
    int *p = 1;
    return p != 0;
}

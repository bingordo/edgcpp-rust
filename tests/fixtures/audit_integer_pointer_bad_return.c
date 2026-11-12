/* C11 constraint violation: nonzero integer return to object pointer. */
int *audit_integer_pointer_bad_return(void) {
    return 1;
}

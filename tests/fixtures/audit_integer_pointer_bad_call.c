/* C11 constraint violation: only integer constant expression zero is null. */
extern int audit_integer_pointer_sink(int *);
int audit_integer_pointer_bad_call(void) {
    return audit_integer_pointer_sink(1);
}

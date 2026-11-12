/* C11 constraint violation: runtime integer cannot implicitly convert to pointer. */
extern int audit_integer_pointer_sink(int *);
int audit_integer_pointer_bad_runtime(int value) {
    return audit_integer_pointer_sink(value);
}

/* C11 integer null pointer constants in calls, returns, and assignment. */
extern int audit_integer_pointer_sink(int *);
int audit_integer_pointer_call(void) {
    return audit_integer_pointer_sink(0);
}
int *audit_integer_pointer_return(void) {
    return 0;
}
int audit_integer_pointer_assign(void) {
    int *p = 0;
    return p == 0;
}

/* ISO C11: zero-parameter prototypes must retain function signature metadata. */
int audit_routine_zero(void) { return 41; }
int audit_routine_indirect(int (*fn)(void)) { return fn() + 1; }

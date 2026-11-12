/* Constraint violation: dropping volatile from an actual pointer argument. */
extern int audit_cv_mutable_sink(int *);
int audit_cv_bad_volatile(volatile int *p)
{
    return audit_cv_mutable_sink(p);
}

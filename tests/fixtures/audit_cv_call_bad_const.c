/* Constraint violation: dropping const from an actual pointer argument. */
extern int audit_cv_mutable_sink(int *);
int audit_cv_bad_const(const int *p)
{
    return audit_cv_mutable_sink(p);
}

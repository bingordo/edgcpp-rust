/* Constraint violation: unsafe qualification insertion below first indirection. */
extern int audit_cv_nested_sink(const int **);
int audit_cv_bad_nested(int **p)
{
    return audit_cv_nested_sink(p);
}

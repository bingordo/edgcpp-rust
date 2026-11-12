/* Constraint violation: dropping const from a returned object pointer. */
int *audit_cv_bad_drop_return(const int *p)
{
    return p;
}

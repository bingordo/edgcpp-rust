/* Constraint violation: dropping const in a pointer assignment. */
int audit_cv_bad_drop_assign(const int *p)
{
    int *q = p;
    return q != 0;
}

/* Constraint violation: unsafe qualification below two indirections. */
int audit_cv_bad_nested_assign(int **p)
{
    const int **q = p;
    return q != 0;
}

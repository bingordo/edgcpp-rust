/* Constraint violation: unsafe qualification in returned pointer type. */
const int **audit_cv_bad_nested_return(int **p)
{
    return p;
}

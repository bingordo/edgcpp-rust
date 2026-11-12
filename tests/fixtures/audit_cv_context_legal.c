/* Legal C11 pointer conversions outside a direct call argument. */
extern int audit_cv_context_sink(const int *p);

int audit_cv_context_assign(int *p)
{
    const int *q;
    q = p;
    return audit_cv_context_sink(q);
}

const int *audit_cv_context_return(int *p)
{
    return p;
}

int audit_cv_context_const_void(const int *p)
{
    const void *q = p;
    return q != 0;
}

/* Deliberate explicit source cast, not an implicit conversion. */
const int **audit_cv_context_explicit(int **p)
{
    return (const int **)p;
}

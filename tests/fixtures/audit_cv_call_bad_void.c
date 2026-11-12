/* Constraint violation: const object pointer must not become mutable void*. */
extern int audit_cv_mutable_void_sink(void *);
int audit_cv_bad_const_void(const int *p)
{
    return audit_cv_mutable_void_sink(p);
}

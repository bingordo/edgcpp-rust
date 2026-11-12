/* C11: source-legal first-level CV pointer conversions at call sites. */
/* The generated object exports wrappers; GCC supplies the external sinks. */
extern int audit_cv_sink_const(const int *p);
extern int audit_cv_sink_void(const void *p);
extern int audit_cv_sink_volatile(const volatile int *p);
extern int audit_cv_sink_nested(const int **p);

int audit_cv_from_mutable(int *p)
{
    return audit_cv_sink_const(p);
}

int audit_cv_from_const(const int *p)
{
    return audit_cv_sink_const(p);
}

int audit_cv_array_decay(int values[4])
{
    return audit_cv_sink_const(values);
}

int audit_cv_to_const_void(const int *p)
{
    return audit_cv_sink_void(p);
}

int audit_cv_add_volatile(int *p)
{
    return audit_cv_sink_volatile(p);
}

/* Explicit C casts are not implicit argument conversions. The sink inspects
 * only the pointer value, never the pointee reinterpreted by the cast. */
int audit_cv_explicit_nested(int **p)
{
    return audit_cv_sink_nested((const int **)p);
}

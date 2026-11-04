#ifndef M12_QUALIFIED_FIELDS_H
#define M12_QUALIFIED_FIELDS_H
struct m12_fields {
    volatile int v;
    _Atomic int a;
    volatile short h;
    int normal;
};
#endif

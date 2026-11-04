#include "m12_qualified_fields.h"
int m12_field_volatile_twice(struct m12_fields *p) {
    int first=p->v;
    return first+p->v;
}
int m12_field_volatile_write(struct m12_fields *p,int x) {
    p->v=x;
    return p->v;
}
int m12_field_atomic_post(struct m12_fields *p) {
    return p->a++;
}
int m12_field_atomic_pre(struct m12_fields *p) {
    return ++p->a;
}
int m12_field_atomic_add(struct m12_fields *p,int x) {
    return p->a+=x;
}
short m12_field_short(struct m12_fields *p,short x) {
    p->h=x;
    return p->h;
}
int m12_field_plain(struct m12_fields *p,int x) {
    p->normal=x;
    return p->normal;
}

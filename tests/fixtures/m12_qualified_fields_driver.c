#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include "m12_qualified_fields.h"
int m12_field_volatile_twice(struct m12_fields *);
int m12_field_volatile_write(struct m12_fields *,int);
int m12_field_atomic_post(struct m12_fields *);
int m12_field_atomic_pre(struct m12_fields *);
int m12_field_atomic_add(struct m12_fields *,int);
short m12_field_short(struct m12_fields *,short);
int m12_field_plain(struct m12_fields *,int);
static void check(const char *what,int got,int expected) {
    if(got!=expected) {fprintf(stderr,"%s got=%d expected=%d\n",what,got,expected);exit(1);}
}
int main(void) {
    struct m12_fields field={0};
    for(int n=-30;n<=30;n++) {
        field.v=n;
        atomic_store(&field.a,n);
        check("volatile twice",m12_field_volatile_twice(&field),2*n);
        check("volatile write",m12_field_volatile_write(&field,n+7),n+7);
        check("volatile stored",field.v,n+7);
        check("atomic post",m12_field_atomic_post(&field),n);
        check("atomic pre",m12_field_atomic_pre(&field),n+2);
        check("atomic add",m12_field_atomic_add(&field,4),n+6);
        check("atomic stored",atomic_load(&field.a),n+6);
        check("volatile short",m12_field_short(&field,(short)n),n);
        check("plain field",m12_field_plain(&field,n*2),2*n);
    }
    return 0;
}

/* Nonescaping initialized pointer locals are now supported. This
 * address-taken alias still requires memory and pointer-to-pointer ABI
 * lowering, so this negative fixture must continue to fail closed. */
int m14_bad_indirect_int_local(int (*fn)(double),double x) {
    int (*stored)(double)=fn;
    int (**alias)(double)=&stored;
    return (*alias)(x);
}

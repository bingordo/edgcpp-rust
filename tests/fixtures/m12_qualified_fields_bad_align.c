struct __attribute__((packed)) m12_bad_field {
    char prefix;
    _Atomic int a;
};
int m12_bad_field_alignment(struct m12_bad_field *p) {
    return p->a++;
}

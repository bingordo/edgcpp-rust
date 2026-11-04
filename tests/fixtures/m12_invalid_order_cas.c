int m12_bad_failure(_Atomic int *object) {
    int expected=1;
    return __atomic_compare_exchange_n(object,&expected,2,0,3,2);
}

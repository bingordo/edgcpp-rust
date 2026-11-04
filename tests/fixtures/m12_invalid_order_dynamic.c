int m12_bad_dynamic(_Atomic int *object,int order) {
    return __atomic_load_n(object,order);
}

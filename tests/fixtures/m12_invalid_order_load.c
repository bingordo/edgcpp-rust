int m12_bad_acquire_release(_Atomic int *object) {
    return __atomic_load_n(object,3);
}

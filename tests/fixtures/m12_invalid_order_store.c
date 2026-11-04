void m12_bad_release_acquire(_Atomic int *object) {
    __atomic_store_n(object,7,2);
}

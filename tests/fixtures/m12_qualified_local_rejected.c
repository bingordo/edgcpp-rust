/* A pointer-valued atomic automatic object is not supported by M12's
 * integer-only local access semantics. Must fail before ELF emission. */
int m12_bad_local(void) {
    int value=7;
    int * _Atomic atomic_pointer=&value;
    return *atomic_pointer;
}

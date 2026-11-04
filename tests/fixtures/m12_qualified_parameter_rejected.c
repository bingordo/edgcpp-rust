/* Reject a qualified pointer object as an automatic formal parameter.
 * This is distinct from a parameter pointing to a qualified scalar. */
int m12_bad_qualified_formal(int * _Atomic pointer) {
    return *pointer;
}

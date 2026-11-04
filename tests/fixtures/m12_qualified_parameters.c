/* Source-qualified by-value formals have automatic object semantics.
 * Their entry value arrives through C ABI but subsequent reads and writes
 * must not be replaced by ordinary scalar SSA operations. */
int m12_formal_volatile(volatile int value) {
    int old=value;
    value+=3;
    int current=value;
    return old*100+current;
}
int m12_formal_atomic(_Atomic int value) {
    int old=value++;
    value*=3;
    return old*100+value;
}
short m12_formal_volatile_short(volatile short value) {
    int old=value;
    value+=2;
    return (short)(old+value);
}

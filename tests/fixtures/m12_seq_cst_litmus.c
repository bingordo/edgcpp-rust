/* Classic store-buffering litmus for the existing default seq-cst subset.
 * For seq-cst x=1; r=y and y=1; r=x, r0==r1==0 is forbidden.
 * These functions are compiled independently by EDG/Cranelift or GCC.
 */
int m12_sb_left(_Atomic int *x, _Atomic int *y) {
    *x = 1;
    return *y;
}
int m12_sb_right(_Atomic int *x, _Atomic int *y) {
    *y = 1;
    return *x;
}

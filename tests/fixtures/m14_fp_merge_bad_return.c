/* Mixed conditional-return and fallthrough require a general CFG.
 * Keep rejected until that control-flow shape is correctly implemented. */
double m14_bad_if_return_fallthrough(double a,double b) {
    if (a > b) return a;
    return b;
}

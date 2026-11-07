/* INTEGER expression condition requires the general integer source path. */
double m14_ternary_bad_integer_cond(int flag,double x,double y) {
    return flag ? x : y;
}

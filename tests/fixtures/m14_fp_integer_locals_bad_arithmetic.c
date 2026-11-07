/* Native INTEGER operations have not been modeled in this FP subset. */
double m14_il_bad_arithmetic(int n,double x) {
    int copy=n;
    copy=copy+1;
    return x+(double)copy;
}

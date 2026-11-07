/* C int* cannot be silently cast into a FP pointer ABI. */
double m14_fp_pointer_bad_int(int *p) {
    return (double)*p;
}

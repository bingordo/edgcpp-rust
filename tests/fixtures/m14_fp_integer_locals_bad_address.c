/* Integer address-taking and pointer alias escape remain unsupported. */
double m14_il_bad_address(double x) {
    int n=7;
    int *ptr=&n;
    return x+(double)(*ptr);
}

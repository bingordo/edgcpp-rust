/* Imported data symbols are not ordinary callable prototypes. */
extern double m14_ext_data;
double m14_wrap_external_bad_data(double x) {
    return m14_ext_data+x;
}

/* Ordinary C external float/double data loads are now supported.
 * Observable volatile storage still requires explicit volatile access rules,
 * and must not be silently lowered as an ordinary FP memory load. */
extern volatile double m14_ext_data;
double m14_wrap_external_bad_data(double x) {
    return m14_ext_data+x;
}

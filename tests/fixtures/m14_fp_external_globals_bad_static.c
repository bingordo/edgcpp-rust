/* Static own TU storage is not an external data import. */
static float m14_fp_static=1.0f;
float m14_bad_fp_static(float x) {
    return m14_fp_static+x;
}

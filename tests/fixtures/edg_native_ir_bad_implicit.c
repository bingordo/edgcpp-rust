/* Unchecked implicit usual arithmetic conversions are not inferred. */
unsigned int edg_native_bad_implicit(unsigned int x,int y) {
    return x+y;
}

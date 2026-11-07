/* Native division semantics require a separate checked lowering case. */
unsigned int edg_native_bad_div(unsigned int x,unsigned int y) {
    return x/y;
}

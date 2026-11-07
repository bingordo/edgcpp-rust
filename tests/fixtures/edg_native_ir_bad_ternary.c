/* No eager select or CFG substitution for lazy C ?: */
int edg_native_bad_ternary(int x) {return x ? 3 : 7;}

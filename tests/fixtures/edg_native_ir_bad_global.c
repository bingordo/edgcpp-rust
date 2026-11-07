/* Unchecked native globals are not silently compiled. */
int edg_native_bad_global_value=99;
int edg_native_bad_global(void) {return edg_native_bad_global_value;}

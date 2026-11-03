/* Volatile pointer access is supported, but compound volatile updates
 * must preserve one read and one store in order. The current qualified
 * lvalue subset deliberately rejects these until explicit RMW is added. */
extern volatile int *m12_device_pointer;
int m12_unsupported_indirect(void) { return (*m12_device_pointer)++; }

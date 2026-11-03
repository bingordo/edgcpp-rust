/* Qualified indirection needs an explicit volatile lvalue and access width.
 * Do not silently lower this to an ordinary load. */
extern volatile int *m12_device_pointer;
int m12_unsupported_indirect(void) { return *m12_device_pointer; }

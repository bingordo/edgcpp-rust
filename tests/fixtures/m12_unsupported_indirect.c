/* Qualified pointer-to-struct member accesses need native member qualifier
 * provenance and cannot use the integer-qualified scalar shortcut. */
struct Sample { volatile unsigned bits:3; int payload; };
extern struct Sample *m12_device_pointer;
int m12_unsupported_indirect(void) { return m12_device_pointer->bits; }

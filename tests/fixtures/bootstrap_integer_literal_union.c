/* C11 x86-64 LP64: original address constant explicitly cast into
 * a 64-bit integer array slot. The relocation must stay symbolic.
 */
typedef unsigned long bootstrap_word;
union BootstrapIntegerLiteralUnion {
  bootstrap_word slots[4];
  unsigned char bytes[32];
};
union BootstrapIntegerLiteralUnion bootstrap_integer_literal_union = {
  .slots = {(bootstrap_word)"static-cast-object", 0, 0, 0}
};
int bootstrap_integer_literal_keep(void) { return 7; }

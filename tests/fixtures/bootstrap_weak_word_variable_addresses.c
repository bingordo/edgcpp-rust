/* Public GNU C11 LP64 control: weak union definitions with original
 * routine AND data addresses cast explicitly into 64-bit integer slots.
 * All relocation bytes are symbolic zero placeholders; no runtime pointer
 * bits are part of the initializer image.
 */
typedef unsigned long bootstrap_word;
int bootstrap_weak_word_target(void) { return 37; }
int bootstrap_weak_data_target = 19;

union BootstrapWeakWord5 { bootstrap_word slots[5]; unsigned char bytes[40]; };
union BootstrapWeakWord6 { bootstrap_word slots[6]; unsigned char bytes[48]; };
union BootstrapWeakWord8 { bootstrap_word slots[8]; unsigned char bytes[64]; };

__attribute__((weak)) union BootstrapWeakWord5 bootstrap_weak_word5 = {
    .slots = {(bootstrap_word)&bootstrap_weak_word_target, 0, 0, 0,
              (bootstrap_word)&bootstrap_weak_data_target}
};
__attribute__((weak)) union BootstrapWeakWord6 bootstrap_weak_word6 = {
    .slots = {(bootstrap_word)&bootstrap_weak_word_target, 0, 0, 0, 0,
              (bootstrap_word)&bootstrap_weak_data_target}
};
__attribute__((weak)) union BootstrapWeakWord8 bootstrap_weak_word8 = {
    .slots = {(bootstrap_word)&bootstrap_weak_word_target, 0, 0, 0, 0, 0,
              (bootstrap_word)&bootstrap_weak_data_target, 0}
};

_Static_assert(sizeof(union BootstrapWeakWord5)==40, "weak 5 word layout");
_Static_assert(sizeof(union BootstrapWeakWord6)==48, "weak 6 word layout");
_Static_assert(sizeof(union BootstrapWeakWord8)==64, "weak 8 word layout");
int bootstrap_weak_word_keep(void) { return 7; }

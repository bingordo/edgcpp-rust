/* Public GNU C11 weak union static initializers. Each source-owned
 * 64-bit word slot holds the address of an actual C routine: linker
 * relocations must remain symbolic, and weak bindings must survive ELF.
 * Independent GCC and EDG oracles intentionally share this original C.
 */
typedef unsigned long bootstrap_word;
int bootstrap_weak_word_target(void) { return 37; }

union BootstrapWeakWord5 {
    bootstrap_word slots[5];
    unsigned char bytes[40];
};
union BootstrapWeakWord6 {
    bootstrap_word slots[6];
    unsigned char bytes[48];
};
union BootstrapWeakWord8 {
    bootstrap_word slots[8];
    unsigned char bytes[64];
};

__attribute__((weak)) union BootstrapWeakWord5 bootstrap_weak_word5 = {
    .slots = {(bootstrap_word)&bootstrap_weak_word_target, 0, 0, 0, 0}
};
__attribute__((weak)) union BootstrapWeakWord6 bootstrap_weak_word6 = {
    .slots = {(bootstrap_word)&bootstrap_weak_word_target, 0, 0, 0, 0, 0}
};
__attribute__((weak)) union BootstrapWeakWord8 bootstrap_weak_word8 = {
    .slots = {(bootstrap_word)&bootstrap_weak_word_target, 0, 0, 0, 0, 0, 0, 0}
};
_Static_assert(sizeof(union BootstrapWeakWord5) == 40, "weak word 5 layout");
_Static_assert(sizeof(union BootstrapWeakWord6) == 48, "weak word 6 layout");
_Static_assert(sizeof(union BootstrapWeakWord8) == 64, "weak word 8 layout");
int bootstrap_weak_word_keep(void) { return 7; }

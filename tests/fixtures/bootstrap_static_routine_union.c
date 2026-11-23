/* Public C11 SysV x86-64 original-source function address relocation.
 * The initializer is a symbolic 64-bit routine pointer, NOT host bits.
 */
typedef unsigned long bootstrap_word;
int bootstrap_routine_target(void) { return 37; }
union BootstrapRoutineUnion {
    bootstrap_word slots[4];
    unsigned char bytes[32];
};
union BootstrapRoutineUnion bootstrap_routine_union = {
    .slots = {(bootstrap_word)&bootstrap_routine_target, 0, 0, 0}
};
int bootstrap_routine_keep(void) { return 7; }

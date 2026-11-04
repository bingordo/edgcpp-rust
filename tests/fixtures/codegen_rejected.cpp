// M14 positive float control and negative unsupported feature tests.
#if defined(TEST_GLOBAL_READ)
extern int unresolved_global;
extern "C" int rejected(int a) { return a + unresolved_global; }
#elif defined(TEST_INDIRECT_CALL)
extern "C" int rejected(int (*callee)(int), int a) { return callee(a); }
#elif defined(TEST_FLOAT_RETURN)
// M14 positive control: a plain FP return is now supported.
extern "C" double rejected(double a) { return a; }
#elif defined(TEST_FLOAT_LOCAL)
// Branch-dependent FP local update still needs typed FP control-flow SSA.
extern "C" double rejected(double a) {
    double value = a + 1.0;
    if (a > 0.0) value = value + 2.0;
    return value;
}
#elif defined(TEST_VOLATILE_GLOBAL)
extern volatile int volatile_global;
extern "C" int rejected(int a) { return a + volatile_global; }
#else
#error Define an unsupported fixture variant
#endif

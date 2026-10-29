// Negative tests for constructs still unsupported after M5 memory work.
#if defined(TEST_GLOBAL_READ)
extern int unresolved_global;
extern "C" int rejected(int a) { return a + unresolved_global; }
#elif defined(TEST_CALL)
extern "C" int external_callee(int);
extern "C" int rejected(int a) { return external_callee(a); }
#elif defined(TEST_FLOAT_RETURN)
extern "C" double rejected(double a) { return a; }
#elif defined(TEST_VOLATILE_GLOBAL)
extern volatile int volatile_global;
extern "C" int rejected(int a) { return a + volatile_global; }
#else
#error Define an unsupported fixture variant
#endif

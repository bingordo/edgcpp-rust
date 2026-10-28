// Negative tests for constructs that remain genuinely unsupported by M3.
#if defined(TEST_GLOBAL_READ)
extern int unresolved_global;
extern "C" int rejected(int a) { return a + unresolved_global; }
#elif defined(TEST_CALL)
extern "C" int external_callee(int);
extern "C" int rejected(int a) { return external_callee(a); }
#elif defined(TEST_FLOAT_RETURN)
extern "C" double rejected(double a) { return a; }
#elif defined(TEST_POINTER_PARAM)
extern "C" long long rejected(long long* p) { return p ? 12LL : 13LL; }
#else
#error Define an unsupported fixture variant
#endif

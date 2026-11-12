/* ISO C++20: empty-prototype and function pointer call over C ABI. */
extern "C" int audit_cpp_routine_zero() { return 47; }
extern "C" int audit_cpp_routine_indirect(int (*fn)()) {
    return fn() + 1;
}

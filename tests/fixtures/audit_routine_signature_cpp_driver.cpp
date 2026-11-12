#include <cstdio>
extern "C" int audit_cpp_routine_zero();
extern "C" int audit_cpp_routine_indirect(int (*fn)());
int main() {
    if (audit_cpp_routine_zero() != 47 ||
        audit_cpp_routine_indirect(audit_cpp_routine_zero) != 48) {
        std::fputs("native C++ routine signature result mismatch\n", stderr);
        return 1;
    }
    std::puts("native C++ zero-parameter signature pass");
    return 0;
}

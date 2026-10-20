namespace outer {
int count = 4;
extern const int constant_answer = 42;
int uninitialized_global;
int initialized_from_count = count + 5;
int initialized_with_sizeof = count + sizeof(count);
struct Widget { int member; unsigned flags : 3; };
Widget widget{};
int variadic(int first, ...) { return first; }
int overloaded(int value) { return value; }
long overloaded(long value) { return value; }
unsigned long long big_unsigned() { return 18446744073709551615ULL; }
using CountAlias = int;
int references(CountAlias &a, CountAlias &&b) { return a + b; }
int calculate(int value) { if (value > 0) { return value + count; } return count; }
namespace inner {
int nested = 7;
}
}

namespace cpp_bridge_verification {
struct Base { int value; Base() : value(1) {} virtual ~Base() {} };
struct Secondary { int other; };
struct Derived : public Base, virtual public Secondary {
    int field;
    Derived() : Base(), Secondary(), field(7) {}
    int method() const { return field + value; }
    static int static_method() { return 3; }
};
int exercise() {
    Derived d;
    return d.method() + Derived::static_method();
}
}

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
struct Base { int value; Base() : value(1) {} virtual ~Base() {} virtual int virtual_method() const { return value; } };
struct Secondary { int other; };
struct Derived : public Base, virtual public Secondary {
    int field;
    Derived();
    int method() const;
    int virtual_method() const override;
    static int static_method();
};
Derived::Derived() : Base(), Secondary(), field(7) {}
int Derived::method() const { return field + value; }
int Derived::virtual_method() const { return field; }
int Derived::static_method() { return 3; }
struct InitializerExpressionProbe {
    int field;
    explicit InitializerExpressionProbe(int value);
};
InitializerExpressionProbe::InitializerExpressionProbe(int value) : field(value + 3) {}
int exercise() {
    Derived d;
    InitializerExpressionProbe probe(2);
    return d.virtual_method() + d.method() + Derived::static_method() + probe.field;
}
}

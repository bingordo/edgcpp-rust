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

// Broad structured control-flow coverage for the owned EDG statement graph.
// The matching EDG build may lower individual constructs; native tests require
// the traversal to remain valid and preserve representable control expressions.
namespace control_flow_verification {
int branching(int count) {
    int sum = 0;
    for (int i = 0; i < count; ++i) {
        if (i == 3) continue;
        switch (i % 3) {
            case 0: sum += i; break;
            case 1: sum -= i; [[fallthrough]];
            default: sum += 2;
        }
    }
    do {
        --sum;
    } while (sum > 100);
    if constexpr (sizeof(int) >= 2) {
        sum += 1;
    } else {
        sum = 0;
    }
    goto finish;
finish:
    return sum;
}

int ranged() {
    int values[] = { 1, 2, 3 };
    int result = 0;
    for (int item : values) {
        result += item;
    }
    return result;
}

int exception_handling(int x) {
    try {
        if (x < 0) throw x;
        return x * 2;
    } catch (int error) {
        return -error;
    } catch (...) {
        return 0;
    }
}
}

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

// Function-scoped IL ownership: explicit parameters, implicit this,
// shadowed locals, static locals, and for-initializer declarations.
namespace scope_verification {
int shadow(int input, int offset) {
    int value = input;
    {
        int value = offset + 2;
        if (value > 0) {
            int nested = value;
            offset += nested;
        }
    }
    for (int index = 0; index < input; ++index) {
        int iteration = index;
        value += iteration;
    }
    static int calls = 0;
    ++calls;
    return value + offset + calls;
}
struct Receiver {
    int member;
    int read(int multiplier) const;
};
int Receiver::read(int multiplier) const {
    int local = member * multiplier;
    return local;
}
int use_receiver() {
    Receiver receiver{4};
    return receiver.read(3);
}
}

// The declaration point is not necessarily the initializer execution point:
// EDG can lower initialization to a separate stmk_init, and may eliminate
// source-level stmk_decl records for simple scalar variables.
namespace declaration_site_verification {
struct Resource {
    int value;
    explicit Resource(int input) : value(input) {}
    ~Resource() {}
};
int mixed(int seed) {
    int first = seed, second = seed + 1;
    Resource resource(second);
    int total = resource.value;
    {
        int first = total * 2;
        total += first;
    }
    for (int index = 0; index < 2; ++index) {
        Resource nested(index);
        total += nested.value;
    }
    return total + first;
}
}

// Exercise qualified-name reconstruction beyond the old 128-scope cutoff.
namespace deep_0 {
namespace deep_1 {
namespace deep_2 {
namespace deep_3 {
namespace deep_4 {
namespace deep_5 {
namespace deep_6 {
namespace deep_7 {
namespace deep_8 {
namespace deep_9 {
namespace deep_10 {
namespace deep_11 {
namespace deep_12 {
namespace deep_13 {
namespace deep_14 {
namespace deep_15 {
namespace deep_16 {
namespace deep_17 {
namespace deep_18 {
namespace deep_19 {
namespace deep_20 {
namespace deep_21 {
namespace deep_22 {
namespace deep_23 {
namespace deep_24 {
namespace deep_25 {
namespace deep_26 {
namespace deep_27 {
namespace deep_28 {
namespace deep_29 {
namespace deep_30 {
namespace deep_31 {
namespace deep_32 {
namespace deep_33 {
namespace deep_34 {
namespace deep_35 {
namespace deep_36 {
namespace deep_37 {
namespace deep_38 {
namespace deep_39 {
namespace deep_40 {
namespace deep_41 {
namespace deep_42 {
namespace deep_43 {
namespace deep_44 {
namespace deep_45 {
namespace deep_46 {
namespace deep_47 {
namespace deep_48 {
namespace deep_49 {
namespace deep_50 {
namespace deep_51 {
namespace deep_52 {
namespace deep_53 {
namespace deep_54 {
namespace deep_55 {
namespace deep_56 {
namespace deep_57 {
namespace deep_58 {
namespace deep_59 {
namespace deep_60 {
namespace deep_61 {
namespace deep_62 {
namespace deep_63 {
namespace deep_64 {
namespace deep_65 {
namespace deep_66 {
namespace deep_67 {
namespace deep_68 {
namespace deep_69 {
namespace deep_70 {
namespace deep_71 {
namespace deep_72 {
namespace deep_73 {
namespace deep_74 {
namespace deep_75 {
namespace deep_76 {
namespace deep_77 {
namespace deep_78 {
namespace deep_79 {
namespace deep_80 {
namespace deep_81 {
namespace deep_82 {
namespace deep_83 {
namespace deep_84 {
namespace deep_85 {
namespace deep_86 {
namespace deep_87 {
namespace deep_88 {
namespace deep_89 {
namespace deep_90 {
namespace deep_91 {
namespace deep_92 {
namespace deep_93 {
namespace deep_94 {
namespace deep_95 {
namespace deep_96 {
namespace deep_97 {
namespace deep_98 {
namespace deep_99 {
namespace deep_100 {
namespace deep_101 {
namespace deep_102 {
namespace deep_103 {
namespace deep_104 {
namespace deep_105 {
namespace deep_106 {
namespace deep_107 {
namespace deep_108 {
namespace deep_109 {
namespace deep_110 {
namespace deep_111 {
namespace deep_112 {
namespace deep_113 {
namespace deep_114 {
namespace deep_115 {
namespace deep_116 {
namespace deep_117 {
namespace deep_118 {
namespace deep_119 {
namespace deep_120 {
namespace deep_121 {
namespace deep_122 {
namespace deep_123 {
namespace deep_124 {
namespace deep_125 {
namespace deep_126 {
namespace deep_127 {
namespace deep_128 {
namespace deep_129 {
namespace deep_130 {
namespace deep_131 {
namespace deep_132 {
namespace deep_133 {
namespace deep_134 {
namespace deep_135 {
int depth_value = 123;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}

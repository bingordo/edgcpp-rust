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
namespace deep_0::deep_1::deep_2::deep_3::deep_4::deep_5::deep_6::deep_7::deep_8::deep_9::deep_10::deep_11::deep_12::deep_13::deep_14::deep_15::deep_16::deep_17::deep_18::deep_19::deep_20::deep_21::deep_22::deep_23::deep_24::deep_25::deep_26::deep_27::deep_28::deep_29::deep_30::deep_31::deep_32::deep_33::deep_34::deep_35::deep_36::deep_37::deep_38::deep_39::deep_40::deep_41::deep_42::deep_43::deep_44::deep_45::deep_46::deep_47::deep_48::deep_49::deep_50::deep_51::deep_52::deep_53::deep_54::deep_55::deep_56::deep_57::deep_58::deep_59::deep_60::deep_61::deep_62::deep_63::deep_64::deep_65::deep_66::deep_67::deep_68::deep_69::deep_70::deep_71::deep_72::deep_73::deep_74::deep_75::deep_76::deep_77::deep_78::deep_79::deep_80::deep_81::deep_82::deep_83::deep_84::deep_85::deep_86::deep_87::deep_88::deep_89::deep_90::deep_91::deep_92::deep_93::deep_94::deep_95::deep_96::deep_97::deep_98::deep_99::deep_100::deep_101::deep_102::deep_103::deep_104::deep_105::deep_106::deep_107::deep_108::deep_109::deep_110::deep_111::deep_112::deep_113::deep_114::deep_115::deep_116::deep_117::deep_118::deep_119::deep_120::deep_121::deep_122::deep_123::deep_124::deep_125::deep_126::deep_127::deep_128::deep_129::deep_130::deep_131::deep_132::deep_133::deep_134::deep_135 {
int depth_value = 123;
}

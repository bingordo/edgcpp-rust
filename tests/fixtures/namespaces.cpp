namespace outer {
int count = 4;
struct Widget { int member; unsigned flags : 3; };
Widget widget{};
int variadic(int first, ...) { return first; }
unsigned long long big_unsigned() { return 18446744073709551615ULL; }
using CountAlias = int;
int references(CountAlias &a, CountAlias &&b) { return a + b; }
int calculate(int value) { if (value > 0) { return value + count; } return count; }
namespace inner {
int nested = 7;
}
}

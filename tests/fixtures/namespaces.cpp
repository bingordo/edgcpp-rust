namespace outer {
int count = 4;
struct Widget { int member; };
int calculate(int value) { return value + count; }
namespace inner {
int nested = 7;
}
}

namespace outer {
int count = 4;
struct Widget { int member; };
int calculate(int value) { if (value > 0) { return value + count; } return count; }
namespace inner {
int nested = 7;
}
}

#include <cstdio>

extern "C" long long edg_add(long long, long long);
extern "C" long long edg_affine(long long, long long);
extern "C" long long edg_constant();
extern "C" long long edg_choose(long long, long long);
extern "C" long long edg_absolute(long long);
extern "C" long long edg_compare(long long, long long);
extern "C" long long edg_bits(long long, long long);
extern "C" long long edg_locals(long long, long long);
extern "C" long long edg_locals_choose(long long, long long);

int main() {
    long long checksum = 0;
    for (long long a = -25; a <= 25; ++a) {
        for (long long b = -17; b <= 17; ++b) {
            const auto add = edg_add(a, b);
            const auto affine = edg_affine(a, b);
            if (add != a + b || affine != a * 7 - b) return 1;
            const auto choose = edg_choose(a, b);
            const auto abs = edg_absolute(a);
            const auto cmp = edg_compare(a, b);
            const auto bits = edg_bits(a, b);
            const auto expected_choose = a < b ? a + 11 : b - 13;
            const auto expected_abs = a < 0 ? -a : a;
            const auto expected_cmp = (a >= b) + (a != b) * 2;
            const auto expected_bits = (a & b) ^ (a | b);
            if (choose != expected_choose || abs != expected_abs ||
                cmp != expected_cmp || bits != expected_bits) return 3;
            long long tmp = a * 3 + b;
            tmp = tmp ^ (a - b);
            const long long expected_locals = (tmp + a) * 2 - b;
            if (edg_locals(a, b) != expected_locals) return 4;
            const auto expected_locals_choose = a < b
                ? a * 3 + b + a : a * 3;
            if (edg_locals_choose(a, b) != expected_locals_choose) return 5;
            checksum += (add * 3 + affine + choose + abs + cmp + bits +
                         edg_locals(a, b) + edg_locals_choose(a, b)) * (a + 31);
        }
    }
    if (edg_constant() != 42) return 2;
    std::printf("%lld\n", checksum + edg_constant());
    return 0;
}

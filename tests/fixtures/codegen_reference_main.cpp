#include <cstdio>

extern "C" long long edg_add(long long, long long);
extern "C" long long edg_affine(long long, long long);
extern "C" long long edg_constant();

int main() {
    long long checksum = 0;
    for (long long a = -25; a <= 25; ++a) {
        for (long long b = -17; b <= 17; ++b) {
            const auto add = edg_add(a, b);
            const auto affine = edg_affine(a, b);
            if (add != a + b || affine != a * 7 - b) return 1;
            checksum += (add * 3 + affine) * (a + 31);
        }
    }
    if (edg_constant() != 42) return 2;
    std::printf("%lld\n", checksum + edg_constant());
    return 0;
}

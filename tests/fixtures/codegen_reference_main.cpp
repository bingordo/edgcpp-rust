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
extern "C" long long edg_mutable_if(long long, long long);
extern "C" long long edg_no_else(long long, long long);
extern "C" long long edg_while(long long, long long);
extern "C" long long edg_for(long long, long long);
extern "C" long long edg_do_while(long long, long long);
extern "C" long long edg_for_jumps(long long, long long);
extern "C" long long edg_while_jumps(long long, long long);
extern "C" long long edg_do_jumps(long long, long long);

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
            long long current = a + b;
            if (a < b) current = current * 2; else current = current - a;
            if (edg_mutable_if(a, b) != current + b) return 6;
            long long conditional = a + b;
            if (a < b) conditional = conditional * 3;
            if (edg_no_else(a, b) != conditional - b) return 7;
            long long i = a;
            long long loop_result = b + 1;
            while (i < b) {
                loop_result = loop_result + i;
                i = i + 1;
            }
            if (edg_while(a, b) != loop_result + i) return 8;
            long long for_i = a, for_sum = 2;
            for (for_i = a; for_i < b; for_i = for_i + 1) {
                for_sum = for_sum + for_i;
            }
            if (edg_for(a, b) != for_sum + for_i) return 9;
            long long do_total = b, do_i = a;
            do {
                do_total = do_total + do_i;
                do_i = do_i + 1;
            } while (do_i < b);
            if (edg_do_while(a, b) != do_total + do_i) return 10;
            long long for_jump_i = a, for_jump_sum = b;
            for (for_jump_i = a; for_jump_i < b; for_jump_i = for_jump_i + 1) {
                if (for_jump_i == a + 1) continue;
                if (for_jump_i > a + 3) break;
                for_jump_sum = for_jump_sum + for_jump_i;
            }
            if (edg_for_jumps(a, b) != for_jump_sum + for_jump_i) return 11;
            long long while_jump_i = a, while_jump_sum = b;
            while (while_jump_i < b) {
                while_jump_i = while_jump_i + 1;
                if (while_jump_i == a + 1) continue;
                if (while_jump_i > a + 3) break;
                while_jump_sum = while_jump_sum + while_jump_i;
            }
            if (edg_while_jumps(a, b) != while_jump_sum + while_jump_i) return 12;
            long long do_jump_i = a, do_jump_sum = b;
            do {
                do_jump_i = do_jump_i + 1;
                if (do_jump_i == a + 1) continue;
                if (do_jump_i > a + 3) break;
                do_jump_sum = do_jump_sum + do_jump_i;
            } while (do_jump_i < b);
            if (edg_do_jumps(a, b) != do_jump_sum + do_jump_i) return 13;
            checksum += (edg_for_jumps(a, b) + edg_while_jumps(a, b) +
                         edg_do_jumps(a, b) + edg_for(a, b) + edg_do_while(a, b) +
                         edg_mutable_if(a, b) + edg_no_else(a, b) + edg_while(a, b) +
                         add * 3 + affine + choose + abs + cmp + bits +
                         edg_locals(a, b) + edg_locals_choose(a, b)) * (a + 31);
        }
    }
    if (edg_constant() != 42) return 2;
    std::printf("%lld\n", checksum + edg_constant());
    return 0;
}

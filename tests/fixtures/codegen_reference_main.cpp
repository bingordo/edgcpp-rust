#include <cstdio>

extern "C" long long edg_sdiv64(long long, long long);
extern "C" long long edg_srem64(long long, long long);
extern "C" unsigned long long edg_udiv64(unsigned long long, unsigned long long);
extern "C" unsigned long long edg_urem64(unsigned long long, unsigned long long);
extern "C" int edg_sdiv32(int, int);
extern "C" int edg_srem32(int, int);
extern "C" unsigned int edg_udiv32(unsigned int, unsigned int);
extern "C" unsigned int edg_urem32(unsigned int, unsigned int);
extern "C" unsigned long long edg_shl64(unsigned long long, unsigned int);
extern "C" unsigned long long edg_ushr64(unsigned long long, unsigned int);
extern "C" long long edg_sshr64(long long, unsigned int);
extern "C" int edg_sshr32(int, unsigned int);
extern "C" unsigned int edg_shl32(unsigned int, int);
extern "C" unsigned int edg_udiv_local(unsigned int, unsigned int);
extern "C" unsigned int edg_shift_local(unsigned int, int);
extern "C" int edg_short_and(int);
extern "C" int edg_short_or(int);
extern "C" int edg_conditional_mutation(int);
extern "C" int edg_comma_mutation();
extern "C" unsigned int edg_postfix_value();
extern "C" unsigned short edg_compound_unsigned();
extern "C" int edg_for_postfix(int);
extern "C" int edg_conditional_side_effect(int);
extern "C" int edg_nested_ternary(int,int);
extern "C" int edg_unary_not_bits(int);
extern "C" void edg_void_empty(void);
extern "C" void edg_void_branch(long long);
extern "C" signed char edg_i8_identity(signed char);
extern "C" unsigned int edg_u32_identity(unsigned int);
extern "C" short edg_i16_constant(void);
extern "C" unsigned int edg_u32_wrap(unsigned int);
extern "C" signed char edg_i8_promote(signed char);
extern "C" unsigned long long edg_u64_order(unsigned long long, unsigned long long);
extern "C" unsigned int edg_u32_order(unsigned int, unsigned int);
extern "C" unsigned int edg_u32_cast(unsigned long long);
extern "C" long long edg_signed_convert(unsigned int);
extern "C" unsigned long long edg_u64_top_constant();
extern "C" bool edg_bool_assign(int);
extern "C" bool edg_bool_branch(int);
extern "C" bool edg_bool_identity(bool);
extern "C" unsigned char edg_u8_mutable(unsigned char);
extern "C" short edg_i16_branch(short, int);
extern "C" unsigned int edg_u32_while(unsigned int, unsigned int);
extern "C" unsigned short edg_u16_for(unsigned short);
extern "C" unsigned char edg_u8_merge(unsigned char, unsigned char, int);
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
    if (edg_short_and(0) != 0 || edg_short_and(1) != 107) return 66;
    if (edg_short_or(0) != 105 || edg_short_or(1) != 100) return 67;
    if (edg_conditional_mutation(0) != 88 ||
        edg_conditional_mutation(1) != 55) return 68;
    if (edg_comma_mutation() != 99) return 69;
    if (edg_postfix_value() !=
        (250U + 251U*256U + 250U*65536U)) return 70;
    unsigned short expected = 65532;
    expected += 10;
    expected ^= 0x00ff;
    expected /= 3;
    expected <<= 2;
    if (edg_compound_unsigned() != expected) return 71;
    if (edg_for_postfix(0) != 0 || edg_for_postfix(15) != 105) return 72;
    if (edg_conditional_side_effect(0) != 0 ||
        edg_conditional_side_effect(2) != 5) return 73;
    if (edg_nested_ternary(0,1) != 15 || edg_nested_ternary(1,0) != 3 ||
        edg_nested_ternary(1,1) != 9) return 74;
    if (edg_unary_not_bits(0) != 256 ||
        edg_unary_not_bits(7) != 248) return 75;
    if (edg_sdiv64(-5000000001LL, 7LL) != (-5000000001LL / 7LL)) return 50;
    if (edg_srem64(-5000000001LL, 7LL) != (-5000000001LL % 7LL)) return 51;
    if (edg_sdiv64(5000000001LL, -7LL) != (5000000001LL / -7LL)) return 52;
    if (edg_udiv64(0xffffffffffffffffULL, 3ULL) != (0xffffffffffffffffULL / 3ULL)) return 53;
    if (edg_urem64(0xffffffffffffffffULL, 3ULL) != (0xffffffffffffffffULL % 3ULL)) return 54;
    if (edg_sdiv32(-2147483647, 13) != (-2147483647 / 13)) return 55;
    if (edg_srem32(-2147483647, 13) != (-2147483647 % 13)) return 56;
    if (edg_udiv32(0xffffffffU, 7U) != (0xffffffffU / 7U)) return 57;
    if (edg_urem32(0xffffffffU, 7U) != (0xffffffffU % 7U)) return 58;
    if (edg_shl64(0x123456789ULL, 17U) != (0x123456789ULL << 17U)) return 59;
    if (edg_ushr64(0xf000000000000000ULL, 27U) !=
        (0xf000000000000000ULL >> 27U)) return 60;
    if (edg_sshr64(-4097LL, 7U) != (-4097LL >> 7U)) return 61;
    if (edg_sshr32(-4097, 3U) != (-4097 >> 3U)) return 62;
    if (edg_shl32(0x01234567U, 4) != (0x01234567U << 4)) return 63;
    if (edg_udiv_local(0xffffffffU, 7U) != ((0xffffffffU / 7U) % 137U)) return 64;
    if (edg_shift_local(0xfedcba98U, 4) !=
        ((0xfedcba98U >> 4) << 4)) return 65;
    if (edg_bool_assign(0)) return 43;
    if (!edg_bool_assign(3)) return 44;
    if (!edg_bool_assign(-3)) return 45;
    if (edg_bool_branch(1)) return 46;
    if (!edg_bool_branch(-3)) return 47;
    if (edg_bool_branch(-7)) return 48;
    if (!edg_bool_identity(true) || edg_bool_identity(false)) return 49;
    if (edg_u8_mutable(250U) != (unsigned char)254U) return 33;
    if (edg_u8_mutable(0U) != (unsigned char)4U) return 34;
    if (edg_i16_branch(-180, 1) != 20) return 35;
    if (edg_i16_branch(-180, 0) != -280) return 36;
    if (edg_u32_while(0xfedcba98U, 3U) !=
        (0xfedcba98U + 3U * 0xf0000000U)) return 37;
    if (edg_u32_while(0xfedcba98U, 0U) != 0xfedcba98U) return 38;
    if (edg_u16_for(15U) != 105U) return 39;
    if (edg_u16_for(0U) != 0U) return 40;
    if (edg_u8_merge(250U, 3U, 1) != 3U) return 41;
    if (edg_u8_merge(250U, 3U, 0) != 250U) return 42;
    if (edg_u32_wrap(0xfedcba98U) != (0xfedcba98U + 0xf0000000U)) return 25;
    if (edg_i8_promote(-100) != (signed char)((int)-100 + 20)) return 26;
    if (edg_u64_order(0xffffffffffffffffULL, 1ULL) != 0) return 27;
    if (edg_u64_order(1ULL, 0xffffffffffffffffULL) != 1) return 28;
    if (edg_u32_order(0xf0000000U, 1U) != 1U) return 29;
    if (edg_u32_cast(0xabcdef0198765432ULL) != 0x98765432U) return 30;
    if (edg_signed_convert(0xffffffffU) != -1LL) return 31;
    if (edg_u64_top_constant() != 18446744073709551615ULL) return 32;
    edg_void_empty();
    edg_void_branch(-1);
    edg_void_branch(1);
    if (edg_i8_identity(static_cast<signed char>(-97)) != -97) return 21;
    if (edg_i8_identity(static_cast<signed char>(127)) != 127) return 22;
    if (edg_u32_identity(0xf1234567U) != 0xf1234567U) return 23;
    if (edg_i16_constant() != 123) return 24;
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

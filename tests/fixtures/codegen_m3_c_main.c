#include <stdio.h>
int m3_c_lazy_and(int); int m3_c_lazy_or(int);
int m3_c_ternary(int); int m3_c_comma(int);
int m3_c_postfix(int); int m3_c_prefix(int); int m3_c_prefix_dec(int);
unsigned int m3_c_compound(unsigned int,int);
int m3_c_for(int); _Bool m3_c_bool(int);
int m3_c_param_assign(int);
int m3_c_param_post(int);
int m3_c_param_branch(int);
unsigned int m3_c_param_unsigned(unsigned int,int);
int main(void) {
    if (m3_c_lazy_and(0)!=0 || m3_c_lazy_and(1)!=107) return 1;
    if (m3_c_lazy_or(0)!=105 || m3_c_lazy_or(1)!=100) return 2;
    if (m3_c_ternary(0)!=88 || m3_c_ternary(1)!=55) return 3;
    if (m3_c_comma(3)!=12) return 4;
    if (m3_c_postfix(15)!=1516 || m3_c_prefix(15)!=1616 ||
        m3_c_prefix_dec(15)!=1414) return 5;
    if (m3_c_compound(0xffffffffU,-2)!=(0xffffffffU%(unsigned)-2)) return 6;
    if (m3_c_for(15)!=105 || m3_c_for(0)!=0) return 7;
    if (!m3_c_bool(-5) || m3_c_bool(0)) return 8;
    if (m3_c_param_assign(9)!=32) return 9;
    if (m3_c_param_post(3)!=304) return 10;
    if (m3_c_param_branch(-3)!=-2 || m3_c_param_branch(4)!=3) return 11;
    if (m3_c_param_unsigned(0xffffffffU,-2)!=1U) return 12;
    puts("M3-C11 differential OK");
    return 0;
}

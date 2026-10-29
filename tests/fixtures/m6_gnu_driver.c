#include <stdio.h>
extern int m6_gnu_max(int,int);
extern int m6_gnu_side_effect(int *,int);
extern unsigned long long m6_gnu_unsigned(unsigned int);
int main(void)
{
    int value=11;
    int result=m6_gnu_side_effect(&value,7);
    printf("%d %d %d %d %llu\n",m6_gnu_max(-7,12),
           m6_gnu_max(10,-20),result,value,m6_gnu_unsigned(71U));
    return 0;
}

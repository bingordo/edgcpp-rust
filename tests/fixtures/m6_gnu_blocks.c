/* M6: generic straight-line GNU block-valued expressions. */
int m6_gnu_max(int x, int y)
{
    return ({ int lhs = x + 2; int rhs = y + 5;
              lhs > rhs ? lhs : rhs; });
}
int m6_gnu_side_effect(int *ptr, int a)
{
    int result = ({ int first = *ptr; *ptr = first + a;
                    int after = *ptr; after * 2; });
    return result + *ptr;
}
unsigned long long m6_gnu_unsigned(unsigned int v)
{
    return ({ unsigned long long t = (unsigned long long)v + 9ULL;
              t ^ 0x1234ULL; });
}

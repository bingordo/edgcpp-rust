#include <stdio.h>
#include <stdint.h>

short m6_external(short x, unsigned char y) { return (short)(x * 2 + y); }
void m6_touch(unsigned int *p) { *p ^= 0x5au; }
unsigned int m6_zero(void) { return 19u; }
extern long long m6_execute(int);


extern long m6_for_ever_break(int);
extern int m6_for_ever_return(int);
extern int m6_for_ever_continue(int);


extern int m9_goto_forward(int);
extern int m9_goto_backedge(int);
extern int m9_skip_dead(int);
extern int m9_goto_while_exit(int);
extern int m9_goto_for_exit(int);
extern int m9_switch_fallthrough(int);
extern int m9_switch_no_default(int);
extern int m9_switch_selector_once(int);
extern int m9_switch_nested(int,int);
extern int m9_switch_default_middle(int);
extern int m9_switch_all_return(int);
extern int m9_switch_fallthrough_return(int);

int main(void)
{
    printf("%lld %lld %lld\n", m6_execute(-11),
           m6_execute(0), m6_execute(71));
    printf("%ld %ld %ld\n", m6_for_ever_break(0),
           m6_for_ever_break(5), m6_for_ever_break(17));
    printf("%d %d %d\n", m6_for_ever_return(-11),
           m6_for_ever_return(0), m6_for_ever_return(71));
    printf("%d %d %d\n", m6_for_ever_continue(0),
           m6_for_ever_continue(6), m6_for_ever_continue(9));
    printf("%d %d %d\n", m9_goto_forward(-13),
           m9_goto_forward(0), m9_goto_forward(19));
    printf("%d %d %d\n", m9_goto_backedge(0),
           m9_goto_backedge(2), m9_goto_backedge(15));
    printf("%d %d %d\n", m9_skip_dead(-10),
           m9_skip_dead(0), m9_skip_dead(21));
    printf("%d %d %d\n", m9_goto_while_exit(0),
           m9_goto_while_exit(3), m9_goto_while_exit(9));
    printf("%d %d %d\n", m9_goto_for_exit(-5),
           m9_goto_for_exit(3), m9_goto_for_exit(21));
    printf("%d %d %d %d %d %d\n",
           m9_switch_fallthrough(-2), m9_switch_fallthrough(0),
           m9_switch_fallthrough(4), m9_switch_fallthrough(8),
           m9_switch_fallthrough(-5), m9_switch_fallthrough(37));
    printf("%d %d %d %d\n",
           m9_switch_no_default(-1), m9_switch_no_default(3),
           m9_switch_no_default(0), m9_switch_no_default(91));
    printf("%d %d %d\n",
           m9_switch_selector_once(-1), m9_switch_selector_once(2),
           m9_switch_selector_once(17));
    printf("%d %d %d %d %d %d\n",
           m9_switch_nested(1,2), m9_switch_nested(1,3),
           m9_switch_nested(1,8), m9_switch_nested(4,2),
           m9_switch_nested(9,3), m9_switch_nested(-1,19));
    printf("%d %d %d %d\n",
           m9_switch_default_middle(0),m9_switch_default_middle(2),
           m9_switch_default_middle(3),m9_switch_default_middle(99));
    printf("%d %d %d %d %d\n",
           m9_switch_all_return(-3),m9_switch_all_return(0),
           m9_switch_all_return(4),m9_switch_all_return(17),
           m9_switch_all_return(-11));
    printf("%d %d %d %d\n",
           m9_switch_fallthrough_return(-1),
           m9_switch_fallthrough_return(0),
           m9_switch_fallthrough_return(3),
           m9_switch_fallthrough_return(15));
    return 0;
}

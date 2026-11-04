#include <stdio.h>
#include <stdlib.h>
double m14_local_init(double,double);
float m14_local_assign(float,float);
double m14_local_two(double,double);
double m14_local_stack(double,double,double,double,double,double,double,double,double,double);
float m14_local_demote(float,double);
static void check(int ok, const char *name) {
  if (!ok) { fprintf(stderr,"M14 locals mismatch: %s\n",name);exit(1); }
}
int main(void) {
  check(m14_local_init(3.0,4.25)==12.75,"initialization");
  check(m14_local_assign(4.25f,2.5f)==6.75f,"reassignment");
  check(m14_local_two(9.5,3.25)==6.25,"two locals");
  check(m14_local_stack(1,2,3,4,5,6,7,8,9.25,10.5)==19.75,"overflow stack");
  check(m14_local_demote(2.25f,8.5)==6.25f,"local narrowing");
  return 0;
}

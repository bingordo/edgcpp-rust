#include <stdio.h>
int m5_vector[6] = {3,9,27,81,12,40};
int *m5_alias = m5_vector;
unsigned short m5_grid[3][4] = {
    {1,2,3,4},{5,6,7,8},{9,10,11,12}
};
extern int m5_get(int);
extern int m5_grid_get(int,int);
extern void m5_set(int *,int);
extern void m5_add(unsigned short *,unsigned short);
extern int m5_local(int);
extern int m5_pointer_offset(int *,int);
extern int m5_row_sum(unsigned short (*)[4],int);
extern int m5_stack_array(int);
extern int m5_global_pointer(int);
extern int m5_post_index(int *,int);
extern int m5_pre_index(int *,int);
extern long long m5_pointer_difference(int *,int *);
extern int m5_pointer_less(int *,int *);
int main(void) {
    printf("%d %d %d %d\n",m5_get(2),m5_grid_get(2,3),m5_local(10),
      m5_row_sum(m5_grid,1));
    m5_set(&m5_vector[4],123);
    m5_add(&m5_grid[1][2],17);
    printf("%d %d %d\n",m5_get(4),m5_grid_get(1,2),
      m5_pointer_offset(m5_vector,5));
    printf("%d\n",m5_stack_array(22));
    printf("%d %d %d %lld %d\n",m5_global_pointer(2),
      m5_post_index(m5_vector,0),m5_pre_index(m5_vector,1),
      m5_pointer_difference(m5_vector,&m5_vector[5]),
      m5_pointer_less(m5_vector,&m5_vector[5]));
    return 0;
}

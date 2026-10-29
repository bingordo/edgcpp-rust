/* M5: typed pointers, 1D/2D indexing, aliases, address-taken local. */
extern int m5_vector[6];
extern unsigned short m5_grid[3][4];
int m5_get(int index) { return m5_vector[index]; }
int m5_grid_get(int row, int col) { return m5_grid[row][col]; }
void m5_set(int *p, int v) { *p = v; }
void m5_add(unsigned short *p, unsigned short v) { *p += v; }
int m5_local(int x) {
    int local = x + 3;
    int *p = &local;
    *p += 7;
    return local;
}
int m5_pointer_offset(int *p, int i) { return *(p+i); }
int m5_row_sum(unsigned short (*p)[4], int row) {
    return p[row][0] + p[row][3];
}

int m5_stack_array(int x) {
    unsigned short values[3];
    values[0] = (unsigned short)x;
    values[1] = (unsigned short)(values[0] + 2);
    values[2] = (unsigned short)(values[1] + 3);
    return (int)values[2];
}

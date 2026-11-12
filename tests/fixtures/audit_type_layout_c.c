/* Native complete and incomplete layouts, fixed and runtime arrays. */
struct AuditOpaque;
struct AuditPair { int x; int y; };
static int fixed_values[4] = {1,2,3,4};
int main(void) {
    struct AuditOpaque *opaque = 0;
    struct AuditPair pair = {5,6};
    int n = 3;
    int runtime_values[n];
    runtime_values[0]=7;
    runtime_values[1]=8;
    runtime_values[2]=9;
    return (opaque == 0 && fixed_values[3] == 4 &&
            pair.x + pair.y == 11 && runtime_values[n-1]==9) ? 0 : 1;
}

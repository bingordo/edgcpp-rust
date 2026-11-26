/* GNU C: real statement-expression in a for increment and local init.
 * GCC oracle checks declaration scope, branches, side effects and result. */
static int probe(int n) {
    int i=0, total=0, calls=0;
    for (;i<n; ({ int next=i+1; ++calls; i=next; })) {
        int v=({ int x=i*2; if (i&1) x+=3; x; });
        total+=v;
    }
    return total+10*calls;
}
int main(void) { return probe(4)!=58; }

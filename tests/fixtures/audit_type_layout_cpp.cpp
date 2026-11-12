/* Native C++20 complete/incomplete layouts and fixed bounds. */
struct AuditOpaque;
struct AuditPair { int x; int y; };
static int fixed_values[5] = {1,2,3,4,5};
int main() {
    AuditOpaque *opaque = nullptr;
    AuditPair pair{6,7};
    return (opaque==nullptr && fixed_values[4]==5 &&
            pair.x+pair.y==13) ? 0 : 1;
}

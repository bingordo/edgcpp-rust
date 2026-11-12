/* C++20 source oracle for native integer, FP and i64 metadata. */
static long audit_long = -19L;
static unsigned int audit_unsigned = 23U;
static float audit_float = 1.25f;
static double audit_double = 2.5;
static bool audit_bool = true;
int main() {
    return (sizeof(audit_long)==8 && audit_long==-19L &&
            audit_unsigned==23U && audit_float==1.25f &&
            audit_double==2.5 && audit_bool) ? 0 : 1;
}

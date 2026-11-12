/* C11 source oracle for EDG's native integer, float and i64 records. */
static long audit_long = -11L;
static unsigned int audit_unsigned = 17U;
static float audit_float = 1.5f;
static double audit_double = 2.25;
static _Bool audit_bool = 1;
int main(void) {
    return (sizeof(audit_long)==8 && audit_long==-11L &&
            audit_unsigned==17U && audit_float==1.5f &&
            audit_double==2.25 && audit_bool) ? 0 : 1;
}

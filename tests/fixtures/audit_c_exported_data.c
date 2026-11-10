/* No function in this translation unit references these objects.
 * Their definitions must nevertheless be exported for a separate object. */
int audit_c_initialized_export = 37;
int audit_c_tentative_export;
extern int audit_c_extern_initialized_export = 73;

int audit_c_unrelated_function(void) {
    return 5;
}

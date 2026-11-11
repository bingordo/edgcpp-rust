/* A native C translation unit with NO defined functions.
 * All externally visible data must remain linkable from another object. */
int audit_data_only_integer = 23;
float audit_data_only_negative_zero = -0.0f;
int audit_data_only_tentative;
struct AuditDataOnlyPair {
    int number;
    int *pointer;
};
struct AuditDataOnlyPair audit_data_only_pair = {
    .pointer=&audit_data_only_integer,
    .number=7
};

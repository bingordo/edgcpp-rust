/* Public C11 oracle for file-scope union string-address initializers.
 * No mrustc-generated source, private include paths, or dynamic init.
 * Repeating the same spelling tests byte content; EDG constant identity,
 * not spelling, determines whether a literal object is shared.
 */
union BootstrapLiteralUnion {
    const char *items[4];
    unsigned char bytes[32];
};
union BootstrapLiteralUnion bootstrap_literal_union = {
    .items = {
        "first-bootstrap",
        "second-bootstrap",
        "first-bootstrap",
        "fourth"
    }
};
// The real mrustc TU also uses null pointer constants alongside an
// addressable literal, which EDG represents as pointer-typed ck_integer zero.
union BootstrapLiteralUnion bootstrap_literal_union_null = {
    .items = { "nullable", 0, 0, 0 }
};
int bootstrap_literal_runtime(void) {
    const char *const *p = bootstrap_literal_union.items;
    return p[0][0] == 'f' && p[0][5] == '-' &&
           p[1][0] == 's' && p[2][0] == 'f' &&
           p[3][0] == 'f' && p[3][5] == 'h' &&
           bootstrap_literal_union_null.items[0][0] == 'n' &&
           bootstrap_literal_union_null.items[1] == 0 &&
           bootstrap_literal_union_null.items[2] == 0 &&
           bootstrap_literal_union_null.items[3] == 0;
}
int main(void) { return bootstrap_literal_runtime() ? 0 : 1; }

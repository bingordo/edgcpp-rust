/* C11 native-IL routine signature cardinality and GCC execution oracle. */
typedef int (*binary_fn)(int, int);
static int one(int a) { return a + 1; }
static int two(int a, int b) { return a + b; }
static int three(int a, int b, int c) { return a + b + c; }
static int via_pointer(binary_fn callback, int a, int b) {
    return callback(a, b);
}
/* Varargs remain parse-only for EDG/Cranelift reference codegen. */
static int variable(int bias, ...) { return bias; }
int main(void) {
    binary_fn fn = two;
    return one(2) == 3 && via_pointer(fn, 4, 5) == 9 &&
           three(1, 2, 3) == 6 && variable(7, 8, 9) == 7 ? 0 : 1;
}

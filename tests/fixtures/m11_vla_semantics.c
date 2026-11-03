/* M11 EDG VLA semantic projection: the runtime bound must be captured at
 * stmk_set_vla_size, independently of the variable's declaration site.
 * This is a parse/graph gate, not yet a Cranelift executable fixture. */
int m11_simple(int n) {
    int data[n + 1];
    for (int i = 0; i <= n; ++i) data[i] = i * 3 + 1;
    return data[n];
}

int m11_side_effect(int n) {
    int count = n;
    int data[++count];
    data[0] = count;
    data[count - 1] = n + 4;
    return data[0] + data[count - 1];
}

int m11_nested(int n) {
    int outer[n + 3];
    outer[0] = 17;
    {
        int inner[n + 2];
        inner[0] = 19;
        outer[0] += inner[0];
    }
    return outer[0];
}

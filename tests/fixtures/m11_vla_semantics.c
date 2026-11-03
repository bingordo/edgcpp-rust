/* M11 executable VLA differential fixture. EDG-lowered runtime bounds,
 * indexing and cleanup must match GCC and satisfy the live-allocation gate. */
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

/* The second VLA dimension must participate in row strides, not only the
 * total allocation size. */
int m11_multidim(int n, int m) {
    int values[n + 1][m + 2];
    for (int i = 0; i <= n; ++i)
        for (int j = 0; j < m + 2; ++j)
            values[i][j] = 7*i + 3*j + 1;
    return values[n][m + 1] + values[0][0] + values[n/2][m/2];
}

/* sizeof must use the saved runtime dimension and never evaluate ++count
 * a second time. */
int m11_sizeof(int n) {
    int count = n;
    int values[++count];
    int runtime_elements = (int)(sizeof values / sizeof values[0]);
    return runtime_elements + count;
}

/* EDG must insert cleanup on both source return edges. */
int m11_early_return(int n) {
    int values[n + 4];
    values[0] = 2*n + 1;
    if ((n & 1) == 0) return values[0] + 4;
    values[1] = n + 3;
    return values[0] + values[1];
}

/* Re-enter one lexical VLA scope repeatedly, including early exits. */
int m11_loop_exits(int n) {
    int sum = 0;
    for (int i = 1; i <= n; ++i) {
        int values[i + 2];
        values[i] = i*3 + 2;
        if (i % 3 == 0) continue;
        if (i % 7 == 0) break;
        sum += values[i];
    }
    return sum;
}

/* Hundreds of alloc/dealloc iterations check that expired allocations do
 * not get reused as live pointer slots across loop backedges. */
int m11_loop_stress(int n) {
    int sum = 0;
    for (int i = 1; i <= n; ++i) {
        int values[(i % 17) + 2];
        values[0] = i;
        sum += values[0];
    }
    return sum;
}

/* Three nested live VLAs: every return edge must release its entire
 * active scope chain in reverse order. */
int m11_deep_return(int n) {
    int outer[n + 1];
    outer[0] = n + 1;
    {
        int middle[n + 2];
        middle[0] = 3 * n;
        if (n & 1) return outer[0] + middle[0];
        {
            int inner[n + 3];
            inner[0] = 5 * n;
            if (n % 3 == 0) return outer[0] + inner[0];
            return outer[0] + middle[0] + inner[0];
        }
    }
}

/* Inner continue edges must free the inner VLA without releasing the outer
 * loop's live VLA. Both allocations are repeated on the next iteration. */
int m11_repeated_nested(int n) {
    int sum = 0;
    for (int i = 1; i <= n; ++i) {
        int outer[i + 1];
        outer[0] = i;
        for (int j = 1; j <= 3; ++j) {
            int inner[i + j + 1];
            inner[0] = i + j;
            if (j == 2 && i % 5 == 0) continue;
            sum += inner[0];
        }
        sum += outer[0];
    }
    return sum;
}

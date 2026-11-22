/* C records are aggregates, not C++ class-lifetime objects.  */
typedef struct Node {
    int value;
    struct Node *next;
} Node;

struct Outer {
    Node node;
    struct { unsigned char a, b; } pair;
    union { int number; unsigned char bytes[4]; } payload;
};

static struct Outer object = {
    { 41, (Node *)0 },
    { 1, 2 },
    { .number = 3 }
};

int c_record_read(void) {
    return object.node.value + object.pair.a + object.payload.number;
}

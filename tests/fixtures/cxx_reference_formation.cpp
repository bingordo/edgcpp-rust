// Native EDG C++17 reference-formation witness. Keep reference binding and
// ordinary address-of in the same source function; they are different EDG
// operators and must never be conflated by downstream lowering.
namespace reference_formation_probe {
int by_reference(int& value) {
    ++value;
    return value;
}
int by_pointer(const int* value) { return *value; }
int exercise(int seed) {
    int local = seed;
    // The first argument binds an lvalue to an lvalue reference.
    // The second argument explicitly takes an ordinary pointer address.
    return by_reference(local) + by_pointer(&local);
}
}
int main() {
    return reference_formation_probe::exercise(4) == 10 ? 0 : 1;
}

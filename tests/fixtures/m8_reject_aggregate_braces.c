/* Dynamic C11 braced aggregate initializers still require executable
 * expression-tree lowering. They must not become zero or stale bytes. */
struct M8NotYetInitialized { int first; long second; };
long m8_braced_initializer(long input)
{
    struct M8NotYetInitialized value = { (int)input, input + 9 };
    return value.first + value.second;
}

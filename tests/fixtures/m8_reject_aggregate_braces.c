/* M8: braced aggregate constants require a structured initializer tree.
 * The backend must reject these rather than writing one scalar word. */
struct M8NotYetInitialized { int first; long second; };
long m8_braced_initializer(void)
{
    struct M8NotYetInitialized value = {0};
    return value.first + value.second;
}

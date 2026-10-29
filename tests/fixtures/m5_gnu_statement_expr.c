/* Explicit negative gate: GNU ({...}) needs nested block-value lowering.
 * Never accept an object that silently drops side effects or the last value.
 */
int m5_gnu_stmt(int x)
{
    return ({ int y = x + 1; y * 2; });
}

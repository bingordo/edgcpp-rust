/* Unsupported: GNU statement-expr containing nested control flow. */
int m5_gnu_stmt(int x)
{
    return ({ int y = x + 1; if (y > 5) y += 2; y * 2; });
}

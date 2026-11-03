/* Optional reference runtime for EDG's lowered C VLA helper ABI.
 * EDG emits __vla_alloc(&pointer_slot, byte_count) and
 * __vla_dealloc(&pointer_slot) at their lexical execution points.
 *
 * The runtime deliberately uses one malloc per allocation. This mirrors the
 * observable semantics but not the optional pooled EDG runtime optimization.
 * malloc's fundamental alignment suffices for ordinary C scalar VLAs.
 * Extended over-aligned element types require an explicit ABI extension.
 */
#include <stddef.h>
#include <stdlib.h>

/* Test-only instrumentation checks EDG's scope cleanup as well as values.
 * The standalone production runtime has no global bookkeeping at all. */
#ifdef M11_VLA_TEST_INSTRUMENTATION
static void *m11_live_stack[4096];
static size_t m11_live_count;
long m11_vla_test_live(void) { return (long)m11_live_count; }
#endif

void __vla_alloc(void *slot, ptrdiff_t bytes)
{
    if (!slot || bytes < 0) abort();
    /* Zero-bound source VLAs are invalid C; tolerate zero only to match the
       EDG reference runtime's one-byte allocation fallback. */
    size_t length = bytes == 0 ? 1 : (size_t)bytes;
    void *storage = malloc(length);
    if (!storage) abort();
    *(void **)slot = storage;
#ifdef M11_VLA_TEST_INSTRUMENTATION
    if (m11_live_count >= sizeof m11_live_stack / sizeof m11_live_stack[0])
        abort();
    m11_live_stack[m11_live_count++] = storage;
#endif
}

void __vla_dealloc(void *slot)
{
    if (!slot) abort();
    void **storage = (void **)slot;
#ifdef M11_VLA_TEST_INSTRUMENTATION
    /* EDG's pooled runtime also depends on well-nested lexical releases. */
    if (!m11_live_count || m11_live_stack[m11_live_count - 1] != *storage)
        abort();
    --m11_live_count;
#endif
    free(*storage);
    *storage = NULL;
}

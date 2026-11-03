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

void __vla_alloc(void *slot, ptrdiff_t bytes)
{
    if (!slot || bytes < 0) abort();
    /* Zero-bound source VLAs are invalid C; tolerate zero only to match the
       EDG reference runtime's one-byte allocation fallback. */
    size_t length = bytes == 0 ? 1 : (size_t)bytes;
    void *storage = malloc(length);
    if (!storage) abort();
    *(void **)slot = storage;
}

void __vla_dealloc(void *slot)
{
    if (!slot) abort();
    void **storage = (void **)slot;
    free(*storage);
    *storage = NULL;
}

/* Explicitly initialized aggregate beyond the former 4096-byte cap.
 * Preserve the actual initializer bytes. */
unsigned char audit_image_4097[4097] = { [4096] = 89 };

int audit_image_too_large(void) {
    return audit_image_4097[4096];
}

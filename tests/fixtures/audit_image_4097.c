/* Explicitly initialized aggregate one byte beyond the 4096-byte cap.
 * It must be rejected, never lowered as a zero-initialized global. */
unsigned char audit_image_4097[4097] = { [4096] = 89 };

int audit_image_too_large(void) {
    return audit_image_4097[4096];
}

/* The exact maximum supported native aggregate image: 4096 bytes.
 * Sparse designated elements must retain zeros and the last byte. */
unsigned char audit_image_4096[4096] = { [0] = 13, [4095] = 89 };

int audit_image_boundary(void) {
    return audit_image_4096[0] + audit_image_4096[2048] +
           audit_image_4096[4095];
}

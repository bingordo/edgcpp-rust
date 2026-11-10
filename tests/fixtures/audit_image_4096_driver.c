#include <stdio.h>

extern unsigned char audit_image_4096[4096];
extern int audit_image_boundary(void);

int main(void) {
    printf("%d %u %u %u\n", audit_image_boundary(),
           (unsigned)audit_image_4096[0],
           (unsigned)audit_image_4096[2048],
           (unsigned)audit_image_4096[4095]);
    return 0;
}

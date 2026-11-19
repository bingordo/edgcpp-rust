/* Native C11 designated-initializer depth witness.
 * All extents are 1: only a four-byte image is needed, so this exercises
 * traversal depth, not the independent 4 KiB image-size policy.
 */
int deep_designated_array[1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1][1] = { [0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0] = 37 };

struct Leaf { int value; };
struct N1 { struct Leaf inner; };
struct N2 { struct N1 inner; };
struct N3 { struct N2 inner; };
struct N4 { struct N3 inner; };
struct N5 { struct N4 inner; };
struct N6 { struct N5 inner; };
struct N7 { struct N6 inner; };
struct N8 { struct N7 inner; };
struct N9 { struct N8 inner; };
struct N10 { struct N9 inner; };
struct N11 { struct N10 inner; };
struct N12 { struct N11 inner; };
struct N13 { struct N12 inner; };
struct N14 { struct N13 inner; };
struct N15 { struct N14 inner; };
struct N16 { struct N15 inner; };
struct N17 { struct N16 inner; };
struct N18 { struct N17 inner; };
struct N19 { struct N18 inner; };
struct N20 { struct N19 inner; };
struct N21 { struct N20 inner; };
struct N22 { struct N21 inner; };
struct N23 { struct N22 inner; };
struct N24 { struct N23 inner; };
struct N25 { struct N24 inner; };
struct N26 { struct N25 inner; };
struct N27 { struct N26 inner; };
struct N28 { struct N27 inner; };
struct N29 { struct N28 inner; };
struct N30 { struct N29 inner; };
struct N31 { struct N30 inner; };
struct N32 { struct N31 inner; };
struct N33 { struct N32 inner; };
struct N34 { struct N33 inner; };
struct N35 { struct N34 inner; };
struct N36 { struct N35 inner; };
struct N37 { struct N36 inner; };
struct N38 { struct N37 inner; };
struct N39 { struct N38 inner; };
struct N40 { struct N39 inner; };
struct N40 deep_designated_record = { .inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.value = 53 };

int main(void) {
    return deep_designated_array[0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0] != 37 ||
           deep_designated_record.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.inner.value != 53;
}

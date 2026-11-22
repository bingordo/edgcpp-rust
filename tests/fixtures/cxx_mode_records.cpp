// C++ record proof behavior must not change when C-mode guards are added.
struct Plain {
    int value;
};

struct HasDestructor {
    int value;
    ~HasDestructor() {}
};

Plain plain{41};
namespace audit_names {
extern int external_count;
}

int cpp_external_read() { return audit_names::external_count; }

int cpp_record_read() {
    HasDestructor object{1};
    return plain.value + object.value;
}

// C++ record proof behavior must not change when C-mode guards are added.
struct Plain {
    int value;
};

struct HasDestructor {
    int value;
    ~HasDestructor() {}
};

Plain plain{41};

int cpp_record_read() {
    HasDestructor object{1};
    return plain.value + object.value;
}

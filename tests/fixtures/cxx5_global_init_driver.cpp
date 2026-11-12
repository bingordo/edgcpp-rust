// main is intentionally in a different translation unit from the global.
// The program must observe the constructor before main and the destructor
// after main, including when both TUs are compiled independently.
extern "C" int puts(const char *);
extern "C" int global_init_ready();

int main() {
    if (global_init_ready() != 1) return 21;
    puts("main");
    return 0;
}

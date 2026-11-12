// Cross-translation-unit startup/shutdown fixture. The one dynamic global
// makes constructor/destructor order observable without relying on undefined
// ordering between different translation units.
extern "C" int puts(const char *);

struct GlobalInitProbe {
    GlobalInitProbe();
    ~GlobalInitProbe();
};

static int initialized;
GlobalInitProbe::GlobalInitProbe() {
    initialized = 1;
    puts("ctor");
}
GlobalInitProbe::~GlobalInitProbe() {
    initialized = 2;
    puts("dtor");
}
GlobalInitProbe global_init_probe;
extern "C" int global_init_ready() { return initialized; }

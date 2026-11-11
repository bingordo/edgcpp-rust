// P2.1a: class-source proof must be distinct from POD/triviality.
struct Marker {
  int value;
  explicit Marker(int x);
  ~Marker();
  int read() const;
};
int lifetime_paths(int seed) {
  Marker outer(seed);
  if (seed < 0) {
    Marker inner(seed + 1);
    return outer.read() + inner.read();
  }
  Marker second(seed + 2);
  return outer.read() + second.read();
}
struct PlainCounter { int value; int read() const; };
int ordinary_record(PlainCounter *p) { return p->read(); }
struct VirtualMarker {
  int value;
  VirtualMarker(int);
  virtual ~VirtualMarker();
  virtual int read() const;
};
int virtual_function(VirtualMarker *m) { return m->read(); }

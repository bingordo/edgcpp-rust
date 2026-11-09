/* C++0: source-only linker identity, native lowering not implemented yet. */
namespace alpha {
    int overloaded(int x) { return x+1; }
    int overloaded(long x) { return (int)x+2; }
    namespace beta {
        int nested(int x) {return x*3;}
    }
}
extern "C" int edg_c_abi(int x) {return x+4;}

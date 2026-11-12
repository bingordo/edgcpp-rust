/* C++20 native direct-base proof and G++ execution oracle. */
struct AuditRoot { int x; };
struct AuditDerived : AuditRoot { int y; char tag; };
union AuditChoice { int integer; long wider; };
int main() {
    AuditDerived value{{1},2,'a'};
    AuditChoice choice{};
    choice.integer=4;
    return (value.x==1 && value.y==2 && value.tag=='a' &&
            choice.integer==4) ? 0 : 1;
}

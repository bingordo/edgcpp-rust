/* Qualified read/write requires separate memory-effect contract. */
volatile double m14_init_volatile = 1.5;
double m14_init_volatile_read(double x){return m14_init_volatile+x;}

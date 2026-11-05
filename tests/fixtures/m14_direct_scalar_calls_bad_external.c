/* External FP calls need an import/visibility contract, not a guessed symbol. */
extern double m14_not_defined_here(double x);
double m14_bad_external(double x) {return m14_not_defined_here(x);}

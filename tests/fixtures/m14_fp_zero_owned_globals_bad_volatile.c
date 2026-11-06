/* Volatile C global access is observably qualified. */
volatile double m14_volatile_zero;
double m14_bad_volatile(double x) {
    m14_volatile_zero=x;
    return m14_volatile_zero;
}

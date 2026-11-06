/* TLS owned global requires separate TLS relocation and address handling. */
_Thread_local double m14_tls_zero;
double m14_bad_tls(double x) {return m14_tls_zero+x;}

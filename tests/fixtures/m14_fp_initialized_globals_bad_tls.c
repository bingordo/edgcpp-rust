/* Source-owned TLS objects require explicit TLS relocations. */
_Thread_local double m14_init_tls = 1.5;
double m14_init_tls_read(double x){return m14_init_tls+x;}

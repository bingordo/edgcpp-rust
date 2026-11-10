// Defensive depth-limit regression: nested volatile reads prevent
// reassociation or constant folding from masking native EDG traversal.
int audit_deep_expression(volatile int x) {
    return x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x + (x))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))));
}

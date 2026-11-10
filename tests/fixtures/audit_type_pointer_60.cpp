// EDG bridge audit: 60 acyclic native pointer layers.
// These deliberately avoid typedef declarations, which can pre-cache
// intermediate type IDs and hide recursive emission depth.
int ************************************************************moderate_pointer;

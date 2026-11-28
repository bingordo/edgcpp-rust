# EDG backend development

- Do not introduce arbitrary defensive depth, node-count, list-length, or worklist limits on valid input. Straightforward recursion is acceptable: users can increase the available stack when needed. Do not rewrite recursion solely to eliminate it, or replace a recursion limit with another arbitrary cap. Prefer simple algorithms, compact storage, identity/cycle checks, and actual wire-format or arithmetic representability checks. Use iterative traversal where it simplifies the implementation or provides a demonstrated performance or scalability benefit.
- Validate native changes through `bingordo/compiler-ci` rather than spending source-repository CI minutes. Preserve wire semantics and add regression tests.

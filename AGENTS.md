# EDG backend development

- Do not introduce arbitrary defensive depth, node-count, list-length, or worklist limits on valid input. Use iterative traversal, compact storage, identity/cycle checks, and actual wire-format or arithmetic representability checks instead. Do not replace a recursion limit with another arbitrary cap.
- Validate native changes through `bingordo/compiler-ci` rather than spending source-repository CI minutes. Preserve wire semantics and add regression tests.

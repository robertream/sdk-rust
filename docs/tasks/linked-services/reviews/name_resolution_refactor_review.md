# Code Review: NameResolution Trait Refactor

**Date**: 2026-05-27
**Branch**: `linked-services`
**Scope**: Replace runtime HandlerResolver closure with trait-based NameResolution<S> + NamedService

---

## Summary Assessment

The refactoring is well-structured. The two-trait design is sound, orphan rules are respected, old code is fully cleaned up. One critical issue: the generated resolver dropped the `strip_prefix` validation from the prototype.

## Scores (0-10)

| Category | Score |
|----------|-------|
| Security Posture | 9 |
| Logic Correctness | 7 |
| Code Quality | 8 |
| Production Readiness | 7 |

---

## Findings

### CRITICAL

1. **Generated resolver drops `strip_prefix` validation** (`macros/src/generator.rs:569-570`)

   Generated code uses `full.rsplit("::").next()?` without validating the type prefix.
   The prototype (`tests/named_service_prototype.rs:86-102`) uses `strip_prefix('<')?.strip_prefix(service)?.strip_prefix(" as ")?` to verify the function belongs to `T`.

   Without this, any function ending in `::handler_name` could match — including free functions with compatible signatures.

   **Fix**: Port the `strip_prefix` logic from the prototype into the generated code.

### HIGH

2. **`.expect("completion handler not found")` panics without context** (`src/context/request.rs:339`)

   If resolution fails, the panic message doesn't include the function or service type name.

   **Fix**: Use `unwrap_or_else` with `type_name::<F>()` and `type_name::<S>()` in the message.

### MEDIUM

3. **Resolver only generated for Object types** (`macros/src/generator.rs:550`)
   
   `if self.service_ty != ServiceType::Object { return quote! {}; }` — workflows are excluded. Fine for now since `on_completion` is only used with objects, but should be documented.

### LOW

4. **`type_name` stability not documented** — Add a comment in `handler.rs` noting `type_name` format is not guaranteed by the spec.

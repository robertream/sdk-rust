# Review: resolve() and Invocation Refactoring

**Date**: 2026-04-06
**Scope**: `InvocationHandle::resolve()`, `Invocation` type, removal of context-level result methods

## Summary

Clean refactoring. `InvocationHandle` (lazy) → `resolve()` → `Invocation` (resolved) is intuitive. `&self` on `resolve()` is correct. Two actionable issues.

## Findings

### HIGH

**C1. `get_result` removed but orphaned doc comment remains** — `src/context/mod.rs` had leftover doc. Already cleaned up in latest commit.

**C2. `Invocation` not exported in prelude** — Users can't type-annotate `Invocation` without `use restate_sdk::endpoint::Invocation`. Add to prelude and context re-exports.

### MEDIUM

**R1. `InvocationIdBackedInvocationHandle::cancel` missing `maybe_flip_span_replaying_field()`** — Pre-existing, every other syscall site calls it.

**R2. `Invocation` should derive `Clone` and `Debug`** — Both fields support it.

**R6. No tests for `resolve()` or `Invocation::result()`** — Deferred to integration testing.

### LOW

**R3. `InvocationIdBackedInvocationHandle` structurally identical to `Invocation`** — Could unify.

**R4. Inconsistent `async move { Ok(id) }` vs `ready(Ok(...))`** — Minor style.

**R5. `start_linked` doc doesn't mention `id()` and `result()`** — Update doc.

## Scores

| Dimension | Score |
|---|---|
| API Design | 9 |
| Correctness | 8 |
| Discoverability | 5 |
| Test Coverage | 4 |
| Code Quality | 8 |

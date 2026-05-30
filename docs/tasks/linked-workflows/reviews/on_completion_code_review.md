# Review Analysis: Linked Workflows `on_completion` API - Comprehensive Code Review

**Date**: 2026-04-06
**Branch**: `linked-workflows`
**Scope**: `on_completion`, `start_linked`, `unlink`, `ObjectContextT`, handler resolver, typed context

## Summary Assessment

Well-architected feature with clean dual-path design (default + typed), excellent compile-time error messages via `AsyncFn` bounds, and thorough trybuild test coverage. The macro integration is clean, the typestate pattern (`LinkedRunRequest`) prevents misuse, and the `#[restate_sdk::object(ObjectContextT)]` opt-in is fully backward-compatible.

## Detailed Findings by Severity

### CRITICAL

**C1. `Result<T, TerminalError>` `Deserialize` impl can never produce `Err` variant**
- **File**: `src/serde.rs:343-348`
- The `Deserialize` impl always produces `Ok(T)`. When a linked child **fails**, the server sends failure bytes that will either cause a deserialization error or produce garbage.
- **Recommendation**: Clarify protocol contract. Either change handler input to just `T` (failures via error propagation) or implement proper envelope deserialization. Acceptable as placeholder if server-side is not yet finalized.

**C2. `Serialize` for `Err(TerminalError)` silently discards the error**
- **File**: `src/serde.rs:332-340`
- `Err` variant serializes to empty bytes, losing code and message. May never be called in practice but is a data-loss footgun.
- **Recommendation**: Remove `Serialize` impl if not needed, or serialize error properly.

### HIGH

**H1. `expect` panic in `on_completion` from non-object contexts**
- **File**: `src/context/request.rs:257`
- Default path (`RunRequest::on_completion`) panics at runtime if called from `WorkflowContext` (no resolver set). The `AsyncFn` bound validates signature but not caller context.
- **Recommendation**: Document clearly, recommend typed path. The typed path (`ObjectContextT`) prevents this at compile time.

### MEDIUM

**M1. Duplicated `on_completion` logic between `RunRequest` and `RunRequestT`**
- **File**: `src/context/request.rs:249-265` and `:392-408`
- Identical resolver lookup + type_name extraction. Extract shared helper.

**M2. Handler resolver generated for all objects, not just those using `on_completion`**
- **File**: `macros/src/generator.rs:161-180`
- Negligible overhead (setting a fn pointer). Acceptable for simplicity.

### LOW

**L1. `rsplit("::").next().unwrap()` theoretically panicky**
- Safe in practice (`type_name` never empty), but could use `unwrap_or`.

**L2. Missing module-level docs explaining two-path design**
- Users would benefit from guidance on `ObjectContext` vs `ObjectContextT` trade-offs.

## Comprehensive Scores (0-10)

| Dimension | Score | Notes |
|---|---|---|
| Security Posture | 9 | No security issues; main concern is runtime panic (H1) |
| Logic Correctness | 7 | Core mechanism correct; `Result` serde is placeholder (C1/C2) |
| Code Quality | 8 | Clean architecture, minor DRY violation (M1) |
| Production Readiness | 6 | Needs C1 resolved and e2e integration testing before GA |

## Prioritized Action Plan

1. **C1**: Resolve `Result<T, TerminalError>` deserialization semantics (requires server protocol clarity)
2. **C2**: Fix or remove `Serialize` impl for `Result<T, TerminalError>`
3. **H1**: Document runtime panic risk on default path, recommend typed path
4. **M1**: Extract shared `on_completion` resolver logic
5. **L2**: Add module-level docs for two-path design

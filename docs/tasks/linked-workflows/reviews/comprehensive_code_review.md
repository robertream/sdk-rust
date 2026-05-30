# Linked Workflows — Comprehensive Code Review

## Summary Assessment

The implementation is well-executed, follows existing codebase patterns, and correctly adapts to the shared-core VM API where `sys_send` takes `linked: bool` directly. No critical/blocking issues. Two HIGH, three MEDIUM, four LOW findings.

## Scores (0-10)

- **Security Posture**: 9 — No vulnerabilities, no user input paths
- **Logic Correctness**: 7 — Silent flag loss on call()/send_after() is a real bug
- **Code Quality**: 8 — Clean, follows patterns, minor duplication
- **Production Readiness**: 7 — Path dep must be reverted, call()/send_after() issue

## Files Reviewed

- `Cargo.toml` — shared-core dependency switch
- `src/context/request.rs` — Request struct, linked flag, new_linked()
- `src/endpoint/context.rs` — request_linked(), remove_link(), send() linked path
- `src/context/mod.rs` — ContextClient trait request_linked()
- `macros/src/generator.rs` — run_linked() and unlink() codegen
- `tests/service.rs` — compile tests
- `test-services/src/linked_workflow.rs` — integration test service
- `test-services/src/main.rs` — service registration

## Detailed Findings

### 🔥 HIGH Priority

**H1: Silent `linked` flag loss on `call()` and `send_after()`**
- `src/context/request.rs` — `call()` ignores `self.linked` entirely. `send_after()` hardcodes `false`.
- Writing `run_linked(req).call().await` compiles but never creates a link. Silent failure.
- **Recommendation**: Add `debug_assert!(!self.linked)` in `call()` and `send_after()`, or return a narrower type from `run_linked()` that only exposes `.send()`.

**H2: `request_linked()` exposed on all context types via ContextClient trait**
- `src/context/mod.rs` — `request_linked()` is on the blanket-impl `ContextClient` trait, available to all handler types.
- The macro codegen calls `self.ctx.request_linked()` on `&ContextInternal` directly, so the trait method is redundant.
- **Recommendation**: Remove `request_linked()` from the `ContextClient` trait. The `ContextInternal` method suffices.

### ⚠️ MEDIUM Priority

**M1: Handler identity check uses Rust ident, not Restate name**
- `macros/src/generator.rs` — `self.handler.ident == "run"` should be `self.handler.restate_name == "run"` to handle `#[name = "..."]` correctly.

**M2: No name conflict check for `run_linked`/`unlink`**
- `macros/src/ast.rs` — A workflow trait with a handler named `run_linked` or `unlink` would produce conflicting methods. Should add validation.

**M3: Minor allocation in `unlink()` codegen**
- `macros/src/generator.rs` — `#service_literal.to_string()` works but `String::from(#service_literal)` is idiomatic.

### 💡 LOW Priority

- **L1**: `context.rs` send() error message says "call" (predates this change)
- **L2**: Integration test calls `run_linked()` then immediately `unlink()` — tests compilation not behavior
- **L3**: `new_linked()` duplicates `new()` body; could delegate
- **L4**: Path dep `../sdk-shared-core` must be reverted before publishing

## Prioritized Action Plan

1. **H1**: Fix silent linked flag loss on `call()`/`send_after()` — add assertions or narrow the return type
2. **H2**: Remove `request_linked()` from `ContextClient` trait
3. **M1**: Use `restate_name` instead of `ident` for run handler detection
4. **M2**: Add name conflict checks in AST validation
5. **M3-L4**: Address in cleanup pass

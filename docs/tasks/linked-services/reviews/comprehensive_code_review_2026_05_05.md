# Comprehensive Code Review: Linked Services

**Date**: 2026-05-05
**Branch**: `linked-services`
**Base**: `client-macro`
**Reviewer**: spectre:reviewer agent

---

## 1. Work & File Scope Boundary Validation

**Scope**: Linked workflows with typed completion handlers and promise objects for the Restate Rust SDK.

**In-Scope**: All files in `git diff client-macro...HEAD` (40 files, +2164/-79 lines)

**Out-of-Scope**: `link_object` (planned, uses `sys_link_service` but not yet exposed)

---

## 2. Context Collection Summary

Reviewed task documentation, the VM trait in sdk-shared-core, and all implementation files. The feature enables parent-child service linking with typed completion callbacks and compile-time safety enforcement.

---

## 3. Files Reviewed

- `src/context/mod.rs` — ObjectContextT, WorkflowContextT, PromiseObject, trait hierarchy
- `src/context/request.rs` — StartRequest, StartRequestT, LinkedRunRequest, CompletionHandlerFnT
- `src/endpoint/context.rs` — link(), remove_link(), get_invocation_output(), prepare_send()
- `macros/src/generator.rs` — ObjectClientT generation, handler resolver
- `macros/src/lib.rs`, `macros/src/ast.rs` — context option parsing
- `examples/linked_services.rs` — full API example
- `src/serde.rs`, `src/errors.rs` — Result/TerminalError serde support
- All UI tests in `tests/ui/`

---

## 4. Summary Assessment

**Overall**: Well-designed, production-ready implementation with strong compile-time safety. The main technical debt is reliance on `std::any::type_name` for handler name resolution — it works reliably in practice but is not guaranteed by the language spec.

**Security Posture**: No concerns — no user input handling, no network exposure, no secrets.

**Risk Level**: LOW for deployment. The type_name concern is theoretical, not actively broken.

---

## 5. Detailed Findings by Severity

### 🚨 CRITICAL Issues

None.

### 🔥 HIGH Priority

1. **`type_name` reliance for handler resolution** (`src/context/request.rs:342-344`)

   ```rust
   let name = any::type_name::<F>();
   let method = name.rsplit("::").next().unwrap();
   let completion_handler_name = resolver(method).to_string();
   ```

   `std::any::type_name` has no stability guarantees. While current rustc always produces `path::method_name`, this could change. The `panic!` in the generated resolver (if the name doesn't match) makes this a runtime failure.

   - **Impact**: If rustc changes type_name format, handler resolution silently breaks → runtime panic
   - **Recommendation**: Consider generating a const-based resolution or document as known limitation with integration test coverage

2. **`handler_resolver` panic on mismatch** (`macros/src/generator.rs:199-201`)

   ```rust
   other => panic!("'{}' is not a handler on {}", other, service_literal),
   ```

   - **Impact**: Runtime panic in production if type_name produces unexpected format
   - **Recommendation**: Return an error or log + fallback rather than panic

### ⚠️ MEDIUM Priority

3. **`remove_link` discards completable notification handle** (`src/endpoint/context.rs:690`)

   ```rust
   // sys_unlink_service is now completable but we discard the handle for now
   let _ = match inner_lock.vm.sys_unlink_service(...)
   ```

   - **Benefit**: Awaiting would let callers know if unlink succeeded
   - **Effort**: Low — make `unlink` return a future, or document fire-and-forget semantics

4. **`get_result` vs `get_output` API inconsistency** (`src/context/mod.rs`)

   `WorkflowContext::get_result` takes `&impl InvocationHandle` while `WorkflowContextT::get_output` takes `&Invocation<T>`. Users of non-T contexts can't easily use linked invocations.

   - **Benefit**: Cleaner API surface, less confusion
   - **Effort**: Low — make `Invocation` implement `InvocationHandle`

5. **Significant duplication across context types** (`src/context/mod.rs`)

   Seven context structs with near-identical field sets and From impls. Pre-existing issue amplified by +2 new variants.

   - **Benefit**: Reduced maintenance burden
   - **Effort**: Medium — macro or shared inner struct

### 💡 LOW Priority

6. **Missing `Send` bound documentation** on `ObjectContextT<'ctx, S>`

   - **Benefit**: Clearer error messages for users whose impl types aren't Send
   - **Effort**: Low — add doc comment

7. **`StartRequestT::new` is `pub`** (`src/context/request.rs:268`)

   The `#[doc(hidden)]` is present but the constructor being public means someone could construct it without a handler resolver, hitting the `expect` panic.

   - **Benefit**: Prevents misuse
   - **Effort**: Minimal — already hidden, acceptable risk

---

## 6. Comprehensive Scores (0-10)

| Category | Score | Notes |
|----------|-------|-------|
| **Security Posture** | 9 | No attack surface, internal SDK code |
| **Logic Correctness** | 8 | Sound design, type_name is only fragility |
| **Code Quality** | 8 | Clean patterns, some duplication (pre-existing) |
| **Production Readiness** | 8 | Passes all tests, one theoretical fragility |

---

## 7. Prioritized Action Plan

1. **[Optional]** Add integration test validating type_name resolution works end-to-end
2. **[Optional]** Consider making resolver return Result instead of panicking
3. **[Future]** Make `unlink` async or document fire-and-forget semantics
4. **[Future]** Unify `get_result`/`get_output` API
5. **[Debt]** Reduce context type duplication (separate effort)

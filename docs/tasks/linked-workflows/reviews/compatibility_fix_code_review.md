# Compatibility Fix Code Review: send()/link() Split

**Date**: 2026-04-05
**Branch**: linked-workflows
**Focus**: Splitting `send()` into separate `send()` and `link()` to align with shared-core v0.9.0 API

---

## Summary Assessment

The compatibility fix is **clean, correct, and minimal**. Ready for merge (pending path dependency revert).

- `link()` correctly calls `sys_link()` with proper parameters
- `invocation_id_handle()` extraction reduces duplication cleanly
- `start_linked()` properly delegates to `ctx.link()`
- Macro codegen correctly gates `RunRequest` to workflow `run` handler only
- No critical or high-severity issues found

---

## Scores (0-10)

| Category | Score |
|----------|-------|
| Security Posture | 9 |
| Logic Correctness | 10 |
| Code Quality | 9 |
| Production Readiness | 8 (path dep) |

---

## Findings

### No CRITICAL or HIGH issues

### ⚠️ MEDIUM

**M1: Syscall label "call" used in link error paths** (pre-existing, cosmetic)
- `context.rs:547` serialization error uses `"call"`, and `context.rs:584` uses `syscall: "call"` — would more accurately say `"link"` or `"send"` for those methods. Pre-existing pattern; `send()` also uses `"call"`.

**M2: `remove_link` silently discards errors** (pre-existing)
- `context.rs:599`: `let _ = inner_lock.vm.sys_remove_link(...)` discards `VMResult`. Consider handling with `inner_lock.fail()` in a follow-up.

### 💡 LOW

**L1: Duplicated target/header construction**
- Target construction and header mapping code is identical between `send()` and `link()`. Acceptable duplication given YAGNI — `call()` also has the same pattern.

---

## Verification Checklist

| Check | Status |
|-------|--------|
| `link()` calls `sys_link()` not `sys_send()` | PASS |
| `link()` passes `None` for delay | PASS |
| `send()` no longer has `linked` parameter | PASS |
| `Request::send()`/`send_after()` don't pass linked flag | PASS |
| `start_linked()` calls `ctx.link()` not `ctx.send()` | PASS |
| `remove_link()` unchanged and compatible | PASS |
| Macro generates `RunRequest` only for workflow `run` | PASS |
| Compile tests cover `start_linked()` and `unlink()` | PASS |
| Integration test service exercises both | PASS |
| No trait-level changes needed | PASS |

---

## Known Items (from prior reviews)

- **Path dependency**: `Cargo.toml:40` uses `path = "../sdk-shared-core"`. Must revert to published version before merge.
- **RunRequest delegation**: Manually re-exposes methods from `Request`. Acceptable given small surface area.

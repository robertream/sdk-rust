# Linked Workflows — Code Review v2

**Date**: 2026-04-04 | **Status**: Ready for merge (pending path dep revert)

## Scores (0-10)

- **Security Posture**: 9
- **Logic Correctness**: 9
- **Code Quality**: 9
- **Production Readiness**: 8 (path dep must be reverted)

## First Review Fixes — All Verified

| Finding | Status |
|---------|--------|
| H1: Silent linked flag loss on call/send_after | FIXED — `RunRequest` only exposes `start_linked()`, `Request` has no linked flag |
| H2: request_linked on all contexts | FIXED — removed entirely |
| M1: ident vs restate_name | FIXED — uses `restate_name == "run"` |
| M2: name conflict for unlink | FIXED — moved to context method |

## No Critical or High Issues

## LOW Priority

- **Path dep**: `Cargo.toml` uses `path = "../sdk-shared-core"` — revert to published version before merge
- **RunRequest delegation**: Manually re-exposes 5 methods from `Request`. If `Request` gains new builders, `RunRequest` must update. Acceptable given small surface area.
- **Pre-existing**: `send()` error message says "call" at `context.rs:498`
- **Pre-existing**: `header()` lacks doc comment on both `Request` and `RunRequest`

## Informational

- `unlink()` on `ContextClient` available to all context types (consistent with `workflow_client()` being universal). VM enforces correctness at runtime.
- No `start_linked_after(delay)` — intentional, can be added later.
- VM syscall arguments verified correct against shared-core trait definitions.

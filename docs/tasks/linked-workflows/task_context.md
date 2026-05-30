# Linked Workflows — Task Context

## Feature Summary

Add TDD-driven linked workflow support to the Rust SDK. The user API (decided):

```rust
// Start + link
ctx.workflow_client::<Child>("key").run_linked(req).send();
// With builder options
ctx.workflow_client::<Child>("key").run_linked(req).idempotency_key("abc").send();
// Unlink
ctx.workflow_client::<Child>("key").unlink();
```

- `run_linked()` is macro-generated on workflow clients only, returns `Request` with `linked: true`
- `.send()` checks the flag, emits `OneWayCallCommand` + `CreateLinkCommand`, returns `impl InvocationHandle`
- `unlink()` is on the workflow client, fire-and-forget, emits `RemoveLinkCommand`

## Protocol Changes Required

| Message | Type ID | Completable |
|---|---|---|
| `CreateLinkCommandMessage` | `0x0415` | Yes (Void) |
| `CreateLinkCompletionNotificationMessage` | `0x8015` | — |
| `RemoveLinkCommandMessage` | `0x0422` | No |

### CreateLinkCommandMessage fields
- `child_service_name: string` (1), `child_service_key: string` (2), `result_completion_id: uint32` (4), `name: string` (12)

### RemoveLinkCommandMessage fields
- `child_service_name: string` (1), `child_service_key: string` (2), `name: string` (12)

### CreateLinkCompletionNotificationMessage fields
- `completion_id: uint32` (1), `result: Void` (2)

## Architecture Patterns

### Testing Patterns (no MockVM exists)

- **Ingress tests** (`tests/ingress.rs`): wiremock-based, verify HTTP path/body/headers. Used for testing macro-generated clients and Request builder methods.
- **Integration tests** (`test-services/`, `testcontainers/`): real Restate Docker container, real CoreVM. Command-interpreter pattern for journal verification.
- **Compile tests** (`tests/ui/`): trybuild for macro error messages.
- **No VM mock**: CoreVM is always used directly. The integration boundary is HTTP, not the VM.

### Request Builder Pattern

`Request<'a, Req, Res>` (context/request.rs:74-81) has fields: `ctx`, `request_target`, `idempotency_key`, `headers`, `req`, `res: PhantomData<Res>`. Adding `linked: bool` is straightforward.

Terminal methods: `.call()`, `.send()`, `.send_after(delay)`.

### send() Path

```
Request::send() → ContextInternal::send() → vm.sys_send(target, input, delay, options) → SendHandle
```

`ContextInternal::send()` (context.rs:467-536): locks VM, converts RequestTarget→Target, serializes, calls sys_send, wraps result.

### Macro Codegen

- `client_method_tokens` (generator.rs:526-540): uniform per-handler, generates `fn handler_name(&self, req) -> Request<'ctx, Req, Res>`
- `impl_client_tokens` (generator.rs:345-364): wraps all handler methods in the client impl block
- `context_request_target_tokens` (generator.rs:556-569): workflow branch emits `RequestTarget::workflow(...)`
- `run` handler is NOT detected specially — treated identically to all other handlers
- `IntoWorkflowClient` trait (mod.rs:587-589) is implemented only for workflow clients

### Where to add run_linked()

Option A: In `client_method_tokens` (generator.rs:526), detect `ServiceType::Workflow && handler.ident == "run"` and emit `run_linked()` alongside `run()`.

### Where to add unlink()

In `impl_client_tokens` (generator.rs:345-364), add extra methods for workflow clients after the per-handler methods. `unlink()` would call a new `ContextInternal::unlink()` that emits `RemoveLinkCommand`.

## Key Files

| File | Purpose |
|---|---|
| `src/context/request.rs` | `Request` struct, `RequestTarget`, `.send()/.call()` terminals |
| `src/context/mod.rs` | Context types, sealed traits, `ContextClient`, `IntoWorkflowClient` |
| `src/endpoint/context.rs` | `ContextInternal` — VM syscall wrappers (`send`, `call`, etc.) |
| `macros/src/generator.rs` | Code generation for clients, serve structs, discovery |
| `macros/src/ast.rs` | AST parsing, `ServiceType`, `Handler` |
| `tests/ingress.rs` | Ingress client tests (wiremock) |
| `test-services/src/` | Integration test service implementations |

## Dependencies

- `restate-sdk-shared-core` — currently pinned `=0.7.1` at Cargo.toml:40. Switch to path dep `../sdk-shared-core` (on `linked-workflows` branch) which already has:
  - `sys_create_link(child_service_name, child_service_key, name: Option<String>) -> VMResult<NotificationHandle>`
  - `sys_remove_link(child_service_name, child_service_key, name: Option<String>) -> VMResult<()>`
  - Message types: `CreateLinkCommand = 0x0415`, `CreateLinkCompletionNotification = 0x8015`, `RemoveLinkCommand = 0x0416`
  - Tests in `src/tests/linked_workflows.rs`

## Implementation Approach (TDD)

### Test surfaces

1. **`tests/service.rs`** — macro compile tests. Verify `run_linked()` and `unlink()` exist on workflow clients. Fast, in-process.
2. **`tests/ui/`** — negative compile tests. Verify `run_linked()` does NOT compile on service/object clients.
3. **`test-services/src/`** — new `linked_workflow.rs` test service. Parent workflow calls `run_linked()` on a child, later `unlink()`. Integration test against real Restate via testcontainers.

### TDD Red phase

- `tests/service.rs`: call `run_linked()` on workflow client → won't compile until macro generates it
- `test-services/src/linked_workflow.rs`: service using `run_linked()` / `unlink()` → won't compile until ContextInternal and macro support linking

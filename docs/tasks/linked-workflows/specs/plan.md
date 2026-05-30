# Linked Workflows — Implementation Plan

## Overview

Add linked workflow support to the Rust SDK via TDD. The feature lets a workflow (or virtual object) link to a child workflow so that the parent's completion is blocked until all linked children complete, and cancellation propagates through links.

## Desired End State

```rust
// Start a child workflow and link to it
ctx.workflow_client::<Child>("key").run_linked(req).send();

// With builder options
ctx.workflow_client::<Child>("key").run_linked(req).idempotency_key("abc").send();

// Remove a link (fire-and-forget)
ctx.workflow_client::<Child>("key").unlink();
```

- `run_linked()` is macro-generated on workflow clients only (alongside `run()`)
- `.send()` on a linked request emits `OneWayCallCommand` + `CreateLinkCommand`, returns `impl InvocationHandle`
- `unlink()` on the workflow client emits `RemoveLinkCommand`, returns nothing
- `CreateLinkCompletionNotification` (Void) is handled internally by the VM, not exposed to user

## Out of Scope

- Link-only API (linking to an already-running workflow without starting it)
- Ingress client support (linking is a journal concept, not applicable to external HTTP callers)
- Query link state / list links API
- Bidirectional links, supervision, or group primitives

## Technical Approach

### Phase 1: Red Tests (TDD)

Write failing tests that define the expected API surface before any implementation.

**1a. `tests/service.rs` — Macro compile test**

Add a workflow trait and test that the generated client has `run_linked()` and `unlink()`:

```rust
#[restate_sdk::workflow]
trait LinkedWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
}

#[test]
fn workflow_client_has_run_linked() {
    // This will fail to compile until the macro generates run_linked()
    // Verifies: run_linked() exists, returns Request, .send() works
}
```

**1b. `test-services/src/linked_workflow.rs` — Integration test service**

A parent workflow that links to a child:

```rust
#[restate_sdk::workflow]
#[name = "LinkedWorkflowParent"]
trait LinkedWorkflowParent {
    async fn run(input: String) -> HandlerResult<String>;
}

// Implementation: calls child.run_linked(input).send(), later child.unlink()
```

Both tests will fail to compile — that's the red phase.

### Phase 2: Dependency Update

Switch `restate-sdk-shared-core` from crates.io `=0.7.1` to local path dependency:

```toml
# Cargo.toml line 40
restate-sdk-shared-core = { path = "../sdk-shared-core", features = ["request_identity", "sha2_random_seed", "http"] }
```

This gives us `sys_create_link()` and `sys_remove_link()` on the VM trait.

### Phase 3: Request Builder — `linked` Flag

Add `linked: bool` to `Request<'a, Req, Res>`:

```rust
pub struct Request<'a, Req, Res = ()> {
    ctx: &'a ContextInternal,
    request_target: RequestTarget,
    idempotency_key: Option<String>,
    headers: Vec<(String, String)>,
    linked: bool,  // NEW
    req: Req,
    res: PhantomData<Res>,
}
```

- Default `false` in `Request::new()`
- Add `pub(crate) fn new_linked(...)` that sets `linked: true`
- In `.send()`, check the flag — if true, call both `ContextInternal::send()` and `ContextInternal::create_link()`

### Phase 4: ContextInternal — Link/Unlink Syscalls

Add two methods to `ContextInternal` (`src/endpoint/context.rs`):

**`create_link()`** — calls `vm.sys_create_link(child_service_name, child_service_key, name)`. The `NotificationHandle` returned is for the Void completion; we handle it internally (the send path doesn't need to expose it).

**`remove_link()`** — calls `vm.sys_remove_link(child_service_name, child_service_key, name)`. Fire-and-forget, returns nothing.

The `send()` method needs modification: when `linked: true`, after the `sys_send()` call, also call `sys_create_link()` using the service name and key extracted from the `RequestTarget`.

### Phase 5: Macro Codegen — `run_linked()` and `unlink()`

**`run_linked()`**: In `generator.rs`, detect `ServiceType::Workflow` + handler named `run` in `client_method_tokens()`. Emit an additional method:

```rust
pub fn run_linked(&self, req: Req) -> Request<'ctx, Req, Res> {
    self.ctx.request_linked(RequestTarget::workflow(...), req)
}
```

Where `request_linked()` constructs `Request` with `linked: true`.

**`unlink()`**: In `impl_client_tokens()`, add an extra method for workflow clients:

```rust
pub fn unlink(&self) {
    self.ctx.remove_link(#service_name, &self.key);
}
```

### Phase 6: Green Tests

At this point:
- `tests/service.rs` compile test passes — `run_linked()` and `unlink()` exist
- `test-services/linked_workflow.rs` compiles — integration service is valid
- Run full test suite to verify no regressions

### Phase 7: Negative Compile Tests

Add `tests/ui/run_linked_on_service.rs` — verify that calling `run_linked()` on a service client or object client fails to compile with a clear error.

## Critical Files for Implementation

- `src/context/request.rs` — Add `linked` field to `Request`, `new_linked()` constructor
- `src/endpoint/context.rs` — Add `create_link()`, `remove_link()`, modify `send()` for linked path
- `src/context/mod.rs` — Add `request_linked()` to `ContextClient` trait, `unlink()` support
- `macros/src/generator.rs` — Generate `run_linked()` on workflow clients, `unlink()` on workflow clients
- `Cargo.toml` — Switch shared-core to path dependency
- `tests/service.rs` — Red/green macro compile tests
- `test-services/src/linked_workflow.rs` — Integration test service

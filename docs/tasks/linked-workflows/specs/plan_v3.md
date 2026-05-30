# Promise Objects v3 — Implementation Plan

## Overview

Add promise object support across three repos: server, shared-core, and Rust SDK. Both workflow and object linking use the same `LinkCommandMessage` on the wire (`0x0415`) — the server branches on whether `handler_name` is populated:

- **Workflow linking** (existing) — `handler_name` populated. Server invokes handler + creates link. Link result = handler return.
- **Object linking** (new) — `handler_name` empty. Server creates link only, no handler call. Link result = `ctx.resolve()`.

Server-side, the existing `ServiceInvocationResponseSink::Link` is replaced by a new `LinkChild` outbox message that unifies both paths. Handler completion checks the `ChildOf` record to route `LinkCompletionNotification` — no response sink needed.

## Desired End State

```rust
// Promise object with resolve type
#[restate_sdk::object(PromiseContextT)]
pub trait Order {
    type Output = OrderSummary;
    async fn create(input: String) -> HandlerResult<()>;
    async fn finalize() -> HandlerResult<()>;
}

// Parent: create link (no handler call)
let handle: LinkHandle<Order> = ctx
    .linked_object::<OrderClient>("order-789")
    .link()
    .await?;

// Parent: call handlers separately
ctx.object_client::<OrderClient>("order-789").create(input).send();

// Parent: operations via LinkRef
ctx.link(&handle).output().await?;    // DurableFuture<OrderSummary>
ctx.link(&handle).cancel().await?;
ctx.link(&handle).remove().await?;

// With on_completion
ctx.linked_object::<OrderClient>("order-789")
    .on_completion(Self::on_order_done)
    .link()
    .await?;

// Child: resolve (enforces Self::Output)
ctx.resolve(summary).await?;
```

## Out of Scope

- `ctx.link(&handle).exists()` — deferred
- Post-resolve state immutability local flag — deferred
- Ingress client changes

---

## Repo 1: Restate Server (`../restate`)

### Server Phase 1: `LinkChild` outbox message

Replace `ServiceInvocationResponseSink::Link` with a dedicated outbox message:

```rust
// crates/storage-api/src/outbox_table/mod.rs
enum OutboxMessage {
    // ...existing...
    LinkChild(LinkChild),  // NEW — replaces ServiceInvocationResponseSink::Link
}

struct LinkChild {
    // Link establishment (always)
    owner_service_id: ServiceId,
    child_service_id: ServiceId,
    caller_invocation_id: InvocationId,
    caller_completion_id: CompletionId,
    completion_handler_name: Option<String>,

    // Handler invocation (workflow only, None for objects)
    invocation: Option<HandlerInvocation>,
}

struct HandlerInvocation {
    handler_name: String,
    argument: Bytes,
    headers: Vec<Header>,
    idempotency_key: Option<ByteString>,
    execution_time: Option<MillisSinceEpoch>,
}
```

**Files:**
- `crates/storage-api/src/outbox_table/mod.rs` — add `LinkChild`, `HandlerInvocation`
- `crates/types/src/invocation/mod.rs` — remove `ServiceInvocationResponseSink::Link`

### Server Phase 2: `ApplyLinkCommand` — emit `LinkChild` instead of `ServiceInvocation`

Modify the existing `link_command.rs` to branch on `handler_name`:

```
ApplyLinkCommand::apply:
1. Validate parent is keyed
2. Validate child is keyed
3. Self-link guard
4. Cycle check
5. Write ParentOf(Running(completion_handler_name)) on parent partition
6. If handler_name is populated:
   → enqueue OutboxMessage::LinkChild { invocation: Some(HandlerInvocation{...}), ... }
7. If handler_name is empty:
   → enqueue OutboxMessage::LinkChild { invocation: None, ... }
   (No ServiceInvocation created at all)
```

**Files:**
- `crates/worker/src/partition/state_machine/entries/link_command.rs` — replace ServiceInvocation creation with LinkChild outbox message

### Server Phase 3: `on_link_child` — child partition handler

New handler on the child partition that processes `LinkChild`:

```
on_link_child(msg: LinkChild):
1. If msg.invocation.is_some():
   a. Build ServiceInvocation from HandlerInvocation fields (plain, no Link sink)
   b. Call on_service_invocation() — starts the handler
   c. If fails → send LinkedNotification with error → return
2. Write ChildOf { resolved: false }
3. Send LinkedNotification (success) → parent gets confirmation
```

Invocation starts BEFORE link confirmation — if the handler target is invalid, the link is never established.

**Files:**
- `crates/worker/src/partition/state_machine/mod.rs` — add `on_link_child()`, WAL dispatch for `LinkChild`

### Server Phase 4: Handler completion — check `ChildOf` instead of Link sink

Remove the `ServiceInvocationResponseSink::Link` arm from `send_response_to_sinks()`. Instead, in handler completion path:

```
end_invocation / send_response_to_sinks:
  // After existing sink processing...
  if let Some(parent_link) = get_first_parent(&child_service_id) {
      if !parent_link.resolved {
          send LinkCompletionNotification {
              owner_service_id: parent_link.remote_service_id,
              child_service_id,
              result,
          }
      }
  }
```

This unifies workflow handler completion and `ctx.resolve()` — both check `ChildOf` and send `LinkCompletionNotification`. The `ChildOf` record is already written (by `on_link_child`), so this is a point-read on the child's own partition.

**Files:**
- `crates/worker/src/partition/state_machine/mod.rs` — modify `send_response_to_sinks()`, remove `ServiceInvocationResponseSink::Link` handling, add `get_first_parent` check
- `crates/worker/src/partition/state_machine/mod.rs` — remove ChildOf write + LinkedNotification from `on_service_invocation()` (moved to `on_link_child`)

### Server Phase 5: Tests

- Object link: `LinkCommand` with empty handler → `LinkChild { invocation: None }` → ChildOf created → LinkedNotification → parent confirmed
- Workflow link: `LinkCommand` with handler → `LinkChild { invocation: Some(...) }` → handler starts → ChildOf created → LinkedNotification → handler completes → `get_first_parent` → LinkCompletionNotification
- Object resolve: link → resolve → LinkCompletionNotification → parent receives result
- Handler failure before link confirmation → LinkedNotification with error
- Full flow: link → resolve → remove → GC cascade
- Existing workflow link tests updated for new code path

---

## Repo 2: SDK Shared Core (`../sdk-shared-core`)

### Shared-Core Phase 1: `sys_link_object`

New method on `VM` trait that emits `LinkCommandMessage` with empty handler fields:

```rust
fn sys_link_object(
    &mut self,
    service_name: String,
    key: String,
    name: Option<String>,
    completion_handler_name: Option<String>,
    options: PayloadOptions,
) -> VMResult<LinkHandle>;
```

Emits `LinkCommandMessage` with `handler_name: ""`, `parameter: empty`, no headers/idempotency. Same wire type `0x0415`, same `LinkHandle` return. No proto changes needed.

**Files:**
- `src/lib.rs` — add `sys_link_object` to VM trait
- `src/vm/mod.rs` — implement
- `src/tests/link_object.rs` — happy path + rejection tests

---

## Repo 3: Rust SDK (`../sdk-rust`)

### SDK Phase 1: Revert v2 additions

Remove `#[start]` / `StartRequest` / `ObjectClientT`. Keep everything else.

**Remove:**
- `macros/src/ast.rs` — `is_start`, `is_start_attr()`, validation, implicit workflow `run` detection
- `macros/src/generator.rs` — `object_client_t_tokens()`, ObjectClientT generation
- `src/context/request.rs` — rename `StartRequest` → `RunRequest`, `StartRequestT` → `RunRequestT`, `StartT` → `RunT`
- `src/context/mod.rs` — `IntoObjectClientT` trait, typed `object_client()` on T-suffix contexts
- `tests/ui/start_on_service.*`, `tests/ui/start_on_shared.*`
- Simplify `tests/promise_context_test.rs`

**Keep:**
- `PromiseContextT<S>` + `SealedCanResolve` + `ContextResolve`
- `WorkflowContextT<S>` + `SealedCanUsePromises`
- `resolve()` / `resolve_failure()` on `ContextInternal`
- `RunRequest::start_linked()` / `Invocation<T>` (workflow linking)
- `ContextInternal::link()` / `remove_link()`

### SDK Phase 2: `type Output` and `PromiseObject` trait

1. **`PromiseObject` trait** — `type Output: Serialize + Deserialize`
2. **Macro parsing** — detect `type Output = T;`, require for `PromiseContextT`
3. **Macro generation** — emit `impl PromiseObject for MyImpl { type Output = T; }`
4. **Type-check resolve** — `ContextResolve::resolve()` accepts `S::Output`

### SDK Phase 3: `LinkHandle<S>` and `LinkRef<T>`

1. **`LinkHandle<S: PromiseObject>`** — service_name + key + `PhantomData<fn() -> S>`
2. **`LinkRef<'a, T>`** — `output()`, `cancel()`, `remove()`
3. **`ctx.link(&handle)`** on T-suffix contexts → `LinkRef<S::Output>`

### SDK Phase 4: `linked_object()` builder

1. **`IntoLinkedObject`** trait — macro-generated
2. **`LinkedObjectBuilder<'a, S>`** — `.on_completion()`, `.link()`
3. **`ContextInternal::link_object()`** — calls `sys_link_object`
4. **`ctx.linked_object::<C>(key)`** on T-suffix contexts

### SDK Phase 5: Tests and examples

1. Rewrite `tests/promise_context_test.rs`
2. UI tests for type safety constraints
3. Update `examples/linked_services.rs`

---

## Implementation Order

| Phase | Repo | Description | Depends on |
|-------|------|-------------|-----------|
| R1 | SDK | Revert v2 additions | — |
| R2 | SDK | `type Output` + `PromiseObject` trait | R1 |
| R3 | SDK | `LinkHandle<S>` + `LinkRef<T>` | R2 |
| S1 | Server | `LinkChild` outbox message | — |
| S2 | Server | `ApplyLinkCommand` emits `LinkChild` | S1 |
| S3 | Server | `on_link_child` child partition handler | S2 |
| S4 | Server | Handler completion checks `ChildOf` | S3 |
| S5 | Server | Tests | S4 |
| SC1 | Shared-Core | `sys_link_object` | — |
| SC2 | Shared-Core | Tests | SC1 |
| R4 | SDK | `linked_object()` builder + wire to `sys_link_object` | R3 + SC1 |
| R5 | SDK | Tests + examples | R4 |
| E2E | All | Integration test | All |

SDK R1-R3, Server S1-S5, and Shared-Core SC1-SC2 can proceed in parallel. R4 is the integration point.

---

## Critical Files

| File | Repo | Role |
|------|------|------|
| `crates/storage-api/src/outbox_table/mod.rs` | Server | `LinkChild` + `HandlerInvocation` types |
| `crates/types/src/invocation/mod.rs` | Server | Remove `ServiceInvocationResponseSink::Link` |
| `crates/worker/.../entries/link_command.rs` | Server | Emit `LinkChild` instead of `ServiceInvocation` |
| `crates/worker/.../state_machine/mod.rs` | Server | `on_link_child()`, modify `send_response_to_sinks()` |
| `src/lib.rs` | Shared-Core | `sys_link_object` on VM trait |
| `src/vm/mod.rs` | Shared-Core | `sys_link_object` implementation |
| `src/context/mod.rs` | SDK | `PromiseObject`, `LinkRef`, `linked_object()`, `ctx.link()` |
| `src/endpoint/context.rs` | SDK | `LinkHandle`, `ContextInternal::link_object()` |
| `macros/src/ast.rs` | SDK | `type Output` parsing |
| `macros/src/generator.rs` | SDK | `PromiseObject` + `IntoLinkedObject` generation |

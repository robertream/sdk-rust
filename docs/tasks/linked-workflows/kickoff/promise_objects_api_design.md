---
date: 2026-04-07
git_commit: 56f0718
branch: linked-workflows
repo: sdk-rust
topic: Promise Objects Rust SDK API Design
tags: [promise-objects, api-design, type-system, linked-workflows]
status: draft-v3
---

# Promise Objects — Rust SDK API Design (v3)

## Context

A **promise object** is a virtual object with a parent link (`ChildOf`) that can **resolve** — signaling completion to its parent with a result. After resolving, state becomes immutable (server-enforced + SDK-enforced). See `../restate/docs/design/promise_objects.md` for the full server spec.

## Design Decisions

### 1. `link()` — Simple Link Establishment (MVP)

For MVP, `link()` only creates the parent-child relationship. It does NOT call a handler — the user calls handlers separately. This avoids complexity around start handler semantics, link confirmation timing, and handler return types.

```rust
// Parent creates a link to an existing VO
let child: ChildHandle<OrderSummary> = ctx
    .link::<OrderClient>("order-789")
    .await?;

// Parent calls handlers separately
ctx.object_client::<OrderClient>("order-789")
    .create(input)
    .send();

// ... external calls happen (add_item, etc.) ...

// Await the resolve result
let summary: OrderSummary = ctx.get_output(&child).await?;
```

### 2. `ChildHandle<T>` — Typed Link Handle

`ChildHandle<T>` replaces `Invocation<T>`. It represents a linked child with:
- `id()` — the child's service/invocation ID
- `cancel()` — cancel the child
- Phantom `T` — the expected resolve type, inferred from the promise object's `Output` associated type

Uses `PhantomData<fn() -> T>` to avoid `Send`/`'static` bounds on `T`.

### 3. Promise Object Declaration with `type Output`

Promise objects declare their resolve type via an associated type:

```rust
#[restate_sdk::object(PromiseContextT)]
pub trait Order {
    type Output = OrderSummary;

    async fn create(input: String) -> HandlerResult<()>;
    async fn add_item(item: Item) -> HandlerResult<()>;
    async fn finalize() -> HandlerResult<()>;
    #[shared]
    async fn get_status() -> HandlerResult<String>;
}
```

- `ctx.resolve(val)` enforces `val: Self::Output` at compile time
- `link::<OrderClient>("key")` returns `ChildHandle<Order::Output>`
- Handlers return whatever they want — handler return types are independent from the resolve type

### 4. `PromiseContextT<S>` — Resolve Capability

`PromiseContextT<S>` is a new context struct. It is the only context that implements `SealedCanResolve`. The `resolve()` method is gated at compile time via the sealed trait pattern:

```rust
pub trait ContextResolve<'ctx>: private::SealedContext<'ctx> + private::SealedCanResolve {
    fn resolve(&self, result: S::Output) -> impl Future<Output = Result<(), TerminalError>> + Send;
    fn resolve_failure(&self, msg: impl Into<String>, code: u16) -> impl Future<Output = Result<(), TerminalError>> + Send;
}
```

### 5. `get_output()` — Only on `WorkflowContextT`

`WorkflowContextT::get_output(&child)` awaits the resolve result. Returns a `DurableFuture` compatible with `restate_sdk::select!`. The `T` is inferred from `ChildHandle<T>`.

```rust
// Single child
let summary: OrderSummary = ctx.get_output(&child).await?;

// Multiple children with select!
restate_sdk::select! {
    s = ctx.get_output(&order) => handle_order(s),
    r = ctx.get_output(&agent) => handle_agent(r),
};
```

### 6. Linking Is Opt-In via T-Suffix Contexts

Only T-suffix contexts can call `link()`:

| Context | Can link | Can resolve | Can get_output |
|---|---|---|---|
| `Context` (service) | no | no | no |
| `ObjectContext` | no | no | no |
| `ObjectContextT<S>` | yes | no | no |
| `PromiseContextT<S>` | yes | yes | no |
| `WorkflowContext` | no | no | no |
| `WorkflowContextT<S>` | yes | no | yes |
| Shared contexts | no | no | no |

### 7. What's Removed (vs v2)

- **`#[start]` attribute** — not needed. `link()` doesn't call a handler.
- **`StartRequest` / `StartRequestT`** — revert to `RunRequest`. No special request type needed.
- **`start_linked()`** — replaced by `link()` + separate handler call.
- **`ObjectClientT` wrapper** — not needed since there's no `#[start]` to gate return types.

### 8. Post-Resolve State Immutability

Defense in depth per the server spec:

- **Same handler:** After `ctx.resolve()` succeeds, set a local flag. Subsequent `set()`, `clear()`, `clear_all()` calls return an error without hitting the VM.
- **Other handlers:** Server rejects mutations with `OBJECT_RESOLVED` error code on the journal completion.

### 9. Server Protocol

- `ResolveCommand` / `ResolveCommandMessage` — in service protocol (implemented in shared-core)
- `sys_resolve` — in sdk-shared-core VM trait (implemented)
- `LinkCommand` — reused as-is for `link()` (no handler invocation, just link creation)
- `ChildOf { resolved: bool }` — server-side link table (implemented)
- `LinkCompletionNotification` — sent when `ctx.resolve()` is called, NOT on handler return

Discovery type remains `ServiceType::VirtualObject`.

## Type System Layout

```
SealedContext ─── ContextTimers, ContextClient, ContextAwakeables, ContextSideEffects
  |
  +── SealedCanReadState ─── ContextReadState
  |     |
  |     +── SealedCanWriteState ─── ContextWriteState
  |     |     |
  |     |     +── SealedCanResolve ─── ContextResolve
  |     |     |     +── PromiseContextT<S>  (only context with resolve)
  |     |     |
  |     |     +── ObjectContext
  |     |     +── ObjectContextT<S>
  |     |     +── WorkflowContext ── SealedCanUsePromises ── ContextPromises
  |     |     +── WorkflowContextT<S> ── SealedCanUsePromises ── ContextPromises
  |     |
  |     +── SharedObjectContext
  |     +── SharedWorkflowContext ── SealedCanUsePromises
  |
  +── Context (service)
```

## Parent-Side API

```rust
#[restate_sdk::workflow(WorkflowContextT)]
pub trait Orchestrator {
    async fn run(input: String) -> HandlerResult<String>;
}

impl Orchestrator for OrchestratorImpl {
    async fn run(&self, ctx: WorkflowContextT<'_, Self>, input: String) -> HandlerResult<String> {
        // 1. Create a link (no handler call)
        let child: ChildHandle<OrderSummary> = ctx
            .link::<OrderClient>("order-789")
            .await?;

        // 2. Call handlers separately
        ctx.object_client::<OrderClient>("order-789")
            .create(input)
            .send();

        // 3. Await the resolve result
        let summary: OrderSummary = ctx.get_output(&child).await?;

        Ok(format!("{} items", summary.items.len()))
    }
}
```

## Child-Side API

```rust
#[restate_sdk::object(PromiseContextT)]
pub trait Order {
    type Output = OrderSummary;

    async fn create(input: String) -> HandlerResult<()>;
    async fn add_item(item: Item) -> HandlerResult<()>;
    async fn finalize() -> HandlerResult<()>;
    #[shared]
    async fn get_status() -> HandlerResult<String>;
}

impl Order for OrderImpl {
    async fn create(&self, ctx: PromiseContextT<'_, Self>, input: String) -> HandlerResult<()> {
        ctx.set("input", input);
        Ok(())
    }

    async fn finalize(&self, ctx: PromiseContextT<'_, Self>) -> HandlerResult<()> {
        let summary = build_summary(&ctx).await?;
        ctx.resolve(summary).await?;  // delivers OrderSummary to parent
        Ok(())
    }
}
```

## Server Dependencies

- `ResolveCommand` + `ResolveCommandMessage` in service protocol (done)
- `sys_resolve` in sdk-shared-core VM trait (done)
- `ChildOf { resolved: bool }` on link table (done)
- State mutation rejection when resolved (done)
- `LinkCompletionNotification` fires on `ctx.resolve()`, not handler return (server behavior for promise objects)

## Open Questions

- How does `link()` map to the server protocol? Does it reuse `LinkCommand` with no handler target?
- Should `link()` accept a key only, or also a handler + args for the initial call?
- Does the on_completion pattern still work with `link()` instead of `start_linked()`?
- Should `ChildHandle<T>` also support non-promise-object links (regular workflow linking)?

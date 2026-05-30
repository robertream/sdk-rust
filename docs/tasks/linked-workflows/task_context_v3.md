# Promise Objects v3 — Task Context

## Latest API Design (from conversation)

The API has converged to:

```rust
// 1. Promise object declaration
#[restate_sdk::object(PromiseContextT)]
pub trait Order {
    type Output = OrderSummary;  // resolve type
    async fn create(input: String) -> HandlerResult<()>;
    async fn add_item(item: Item) -> HandlerResult<()>;
    async fn finalize() -> HandlerResult<()>;
}

// 2. Parent links to a VO (no handler call — just link creation)
let handle: LinkHandle<Order> = ctx
    .linked_object::<OrderClient>("order-789")
    .link()
    .await?;

// 3. Parent calls handlers separately
ctx.object_client::<OrderClient>("order-789").create(input).send();

// 4. Operations via ctx.link(&handle) -> LinkRef<T>
ctx.link(&handle).output().await?;   // DurableFuture<Order::Output>
ctx.link(&handle).cancel().await?;
ctx.link(&handle).remove().await?;
ctx.link(&handle).exists().await?;

// 5. With on_completion (ObjectContextT)
ctx.linked_object::<OrderClient>("order-789")
    .on_completion(Self::on_order_done)
    .link()
    .await?;

// 6. Child resolves
ctx.resolve(summary).await?;  // enforces summary: Self::Output
```

## Key Types

- `LinkHandle<S>` — lightweight data: service ID + PhantomData<S> (where S is the trait type)
- `LinkRef<T>` — returned by ctx.link(&handle), resolves S::Output to T. Has output(), cancel(), remove(), exists()
- `PromiseContextT<'ctx, S>` — context with resolve() capability (already implemented)
- `WorkflowContextT<'ctx, S>` — context with linking + output capability (already implemented)

## What Exists (Already Implemented)

- PromiseContextT<S> struct with SealedCanResolve ✓
- WorkflowContextT<S> struct ✓
- ObjectContextT<S> struct ✓
- ContextResolve trait with resolve()/resolve_failure() ✓
- sys_resolve wired to real shared-core syscall ✓
- Macro: #[object(PromiseContextT)], #[workflow(WorkflowContextT)] parsing ✓

## What Needs to Change

### Remove (from v2 implementation)
- #[start] attribute parsing (macros/src/ast.rs)
- StartRequest/StartRequestT (revert to RunRequest)
- ObjectClientT wrapper generation (macros/src/generator.rs)
- IntoObjectClientT trait (src/context/mod.rs)
- start_linked() on request types (src/context/request.rs)
- Invocation<T> → replaced by LinkHandle<S>
- Cancellation (already removed)

### Add
- `type Output` associated type on PromiseContextT trait declarations
- `LinkHandle<S>` type (lightweight, just service ID + phantom)
- `LinkRef<T>` type (borrowed view with context ref, has output/cancel/remove/exists)
- `ctx.linked_object::<C>(key)` method on T-suffix contexts → returns a builder
- Builder with `.on_completion()` and `.link()` methods
- `ctx.link(&handle)` method on T-suffix contexts → returns LinkRef<T>
- Macro: enforce `type Output` on PromiseContextT objects
- Macro: generate linked_object client type
- Update resolve() to enforce Self::Output type

### Server Protocol
- link() maps to LinkCommand (no handler target — just service_name + key)
- output() maps to sys_get_invocation_output or a new attach-to-link syscall
- remove() maps to sys_remove_link
- cancel() maps to sys_cancel_invocation
- exists() — may need new syscall or can be derived

## Files Impacted

- src/context/mod.rs — LinkHandle, LinkRef, linked_object(), ctx.link(), trait changes
- src/context/request.rs — remove start_linked, StartRequest→RunRequest revert
- src/endpoint/context.rs — remove Invocation<T>, add LinkHandle<S>, wire link operations
- macros/src/ast.rs — remove #[start], add type Output parsing
- macros/src/generator.rs — remove ObjectClientT, add linked_object client gen, type Output
- macros/src/lib.rs — remove #[start] attribute registration
- src/lib.rs — update prelude exports
- tests/ — update all tests

## Complexity Assessment

- 8+ files impacted
- New pattern (linked_object builder, LinkRef)
- Associated type on macro-generated trait (new macro feature)
- Multiple remove + add operations
- Crosses macro + runtime + context boundaries
→ COMPREHENSIVE tier

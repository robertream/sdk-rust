# Promise Objects — Implementation Tasks

## Phase 1: Foundation (Sequential)

### [1.1] Rename RunRequest → StartRequest
- [ ] **1.1.1** Rename `RunRequest` to `StartRequest` in `src/context/request.rs`
- [ ] **1.1.2** Rename `RunRequestT` to `StartRequestT` in `src/context/request.rs`
- [ ] **1.1.3** Rename `RunT` trait to `StartT` in `src/context/mod.rs`
- [ ] **1.1.4** Update all references in `src/context/mod.rs`, `src/endpoint/context.rs`, `src/lib.rs` (prelude exports)
- [ ] **1.1.5** Update macro generator (`macros/src/generator.rs`) — RunRequest references, RunT references
- [ ] **1.1.6** Update tests and test-services
- [ ] **1.1.7** Verify: `cargo check --all-features` passes, `cargo test --all-features` passes

### [1.2] Add `#[start]` attribute parsing
- [ ] **1.2.1** Add `is_start: bool` field to `Handler` in `macros/src/ast.rs`
- [ ] **1.2.2** Parse `#[start]` attribute during handler parsing, set `is_start`
- [ ] **1.2.3** Validation: reject `#[start]` + `#[shared]` with compile error
- [ ] **1.2.4** Validation: reject `#[start]` on `ServiceType::Service` with compile error
- [ ] **1.2.5** Mark workflow `run` handler as implicitly `is_start = true`
- [ ] **1.2.6** Verify: `cargo check --all-features` passes

## Phase 2: New Context Types (Parallel after Phase 1)

### [2.1] Add `WorkflowContextT<S>` context struct
- [ ] **2.1.1** Define `WorkflowContextT<'ctx, S>` struct in `src/context/mod.rs` (same fields as WorkflowContext + PhantomData<S>)
- [ ] **2.1.2** Implement sealed traits: `SealedContext`, `SealedCanReadState`, `SealedCanWriteState`, `SealedCanUsePromises`
- [ ] **2.1.3** Add `workflow_client()` method returning `WorkflowClientT` (same pattern as ObjectContextT)
- [ ] **2.1.4** Add `object_client()` method returning typed object client
- [ ] **2.1.5** Implement `From<(&'ctx ContextInternal, InputMetadata)>`
- [ ] **2.1.6** Add `get_result()` method (same as WorkflowContext)
- [ ] **2.1.7** Export in prelude
- [ ] **2.1.8** Verify: `cargo check --all-features` passes

### [2.2] Add `PromiseContextT<S>` context struct with resolve
- [ ] **2.2.1** Add `SealedCanResolve` sealed marker trait in `mod private`
- [ ] **2.2.2** Add `ContextResolve<'ctx>` capability trait with `resolve()` and `resolve_failure()` methods
- [ ] **2.2.3** Define `PromiseContextT<'ctx, S>` struct (same fields as ObjectContextT + PhantomData<S>)
- [ ] **2.2.4** Implement all sealed traits: `SealedContext`, `SealedCanReadState`, `SealedCanWriteState`, `SealedCanResolve`
- [ ] **2.2.5** Add `workflow_client()` and `object_client()` methods (same pattern as ObjectContextT)
- [ ] **2.2.6** Implement `From<(&'ctx ContextInternal, InputMetadata)>`
- [ ] **2.2.7** Add `resolve()` implementation on `ContextInternal` (stub until sys_resolve exists in shared-core)
- [ ] **2.2.8** Export in prelude
- [ ] **2.2.9** Verify: `cargo check --all-features` passes

## Phase 3: Macro Generator Updates (After Phase 2)

### [3.1] Update generator for new context types and `#[start]`
- [ ] **3.1.1** Add `WorkflowContextT` to context type selection in `handler_client_tokens()` — when `ServiceType::Workflow` + `!is_shared` + typed_context flag
- [ ] **3.1.2** Add `PromiseContextT` to context type selection — when `ServiceType::Object` + `!is_shared` + promise_context flag
- [ ] **3.1.3** Update client method return type: `#[start]` handlers return `StartRequest` when called from T-suffix context client
- [ ] **3.1.4** Generate `StartT` trait impl for `#[start]` object handlers (parallel to workflow `run`)
- [ ] **3.1.5** Add `WorkflowContextT` option to `#[workflow()]` macro attribute parsing
- [ ] **3.1.6** Add `PromiseContextT` option to `#[object()]` macro attribute parsing
- [ ] **3.1.7** Verify: `cargo check --all-features` passes, `cargo test --all-features` passes

## Phase 4: Tests (After Phase 3)

### [4.1] Compile tests and examples
- [ ] **4.1.1** Add UI test: `#[start]` on `#[shared]` handler → compile error
- [ ] **4.1.2** Add UI test: `#[start]` on service handler → compile error
- [ ] **4.1.3** Add UI test: `resolve()` not available on `ObjectContextT` → compile error
- [ ] **4.1.4** Add test-service example using `PromiseContextT` with `#[start]` and `resolve()`
- [ ] **4.1.5** Update existing linked_workflow test-service if needed
- [ ] **4.1.6** Verify: `cargo test --all-features` passes

## Dependencies

- `sys_resolve` in sdk-shared-core VM trait — not yet implemented. `ctx.resolve()` will be stubbed until this exists.
- `ResolveCommand` server-side — not yet implemented. SDK can be built ahead of server.

# Linked Workflows — Quick Task Plan

## Agreed Scope

**Objective**: Add `run_linked()` and `unlink()` to workflow clients, backed by `CreateLinkCommand`/`RemoveLinkCommand` protocol messages via TDD.

**In Scope**: `run_linked(req)` on workflow clients, `unlink()` fire-and-forget, `linked: bool` on Request, ContextInternal syscall wrappers, shared-core path dep switch, TDD red-green cycle.

**Out of Scope**: Link-only API, ingress client, query links, bidirectional links, negative compile tests.

**Constraints**: shared-core at `../sdk-shared-core` (linked-workflows branch) already has `sys_create_link`/`sys_remove_link`.

## Research Summary

- **Request struct** (`src/context/request.rs:74-81`): 6 fields, adding `linked: bool` is trivial. `new()` at line 84, terminal methods `send()`/`call()`/`send_after()`.
- **ContextInternal::send()** (`src/endpoint/context.rs:467-536`): locks VM, converts `RequestTarget` → `Target`, serializes, calls `vm.sys_send()`. The `RequestTarget::Workflow { name, key, handler }` destructure gives us `child_service_name` and `child_service_key` for `sys_create_link`.
- **Fire-and-forget pattern** (`context.rs:321-325`): `clear()` is the model for `unlink()` — lock, call VM syscall, flip span, return nothing.
- **Macro codegen** (`macros/src/generator.rs:526-540`): `client_method_tokens()` generates per-handler methods. `impl_client_tokens()` at 345-364 wraps them in `impl` block. Handler identity available via `self.handler.ident`.
- **`run` detection**: not special-cased today. Detect via `service.service_ty == ServiceType::Workflow && !self.handler.is_shared` + handler name check, or via `HandlerType::Workflow` from discovery.
- **ContextClient trait** (`src/context/mod.rs:467-475`): `request()` creates `Request::new()`. Add `request_linked()` alongside it, delegates to `Request::new_linked()`.
- **shared-core VM signatures**: `sys_create_link(child_service_name: String, child_service_key: String, name: Option<String>) -> VMResult<NotificationHandle>`, `sys_remove_link(same params) -> VMResult<()>`.

## Approach Summary

TDD red-green: write tests that call `run_linked()`/`unlink()` (won't compile), then implement bottom-up: dependency → Request → ContextInternal → macro → green tests.

## Implementation Tasks

### Phase 1: Red Tests

#### [1.1] Write compile test for `run_linked()` and `unlink()`
- [ ] **1.1.1** In `tests/service.rs`, add a workflow trait with `run` handler. Add a test function that constructs a workflow client and calls `.run_linked(req)` and `.unlink()` — this will fail to compile.
  - Acceptance: test file references `run_linked()` and `unlink()` on the workflow client
  - Acceptance: `cargo test --lib` fails with compile error about missing methods

#### [1.2] Write integration test service using linked workflows
- [ ] **1.2.1** Create `test-services/src/linked_workflow.rs` with a parent workflow that calls `ctx.workflow_client::<Child>("key").run_linked(req).send()` and `ctx.workflow_client::<Child>("key").unlink()`.
  - Acceptance: service file exists with both `run_linked()` and `unlink()` calls
  - Acceptance: won't compile until implementation is complete
- [ ] **1.2.2** Register the new test service in `test-services/src/main.rs`.
  - Acceptance: service is bound in the endpoint builder

### Phase 2: Implementation

#### [2.1] Switch shared-core to path dependency
- [ ] **2.1.1** In `Cargo.toml` line 40, change `restate-sdk-shared-core` from `version = "=0.7.1"` to `path = "../sdk-shared-core"` keeping the same features.
  - Acceptance: `cargo check` resolves shared-core from local path
  - Acceptance: `sys_create_link` and `sys_remove_link` available on `VM` trait

#### [2.2] Add `linked` flag to Request and `new_linked` constructor
- [ ] **2.2.1** In `src/context/request.rs`, add `linked: bool` field to `Request`. Default `false` in `new()`. Add `pub(crate) fn new_linked()` that sets `linked: true`.
  - Acceptance: `Request` has `linked` field
  - Acceptance: `new_linked()` returns `Request` with `linked: true`

#### [2.3] Add `create_link()` and `remove_link()` to ContextInternal
- [ ] **2.3.1** In `src/endpoint/context.rs`, add `create_link(child_service_name, child_service_key)` that calls `vm.sys_create_link()`. Handle the `NotificationHandle` internally (don't expose it).
  - Acceptance: method calls `sys_create_link` on the VM
- [ ] **2.3.2** Add `remove_link(child_service_name, child_service_key)` following the `clear()` fire-and-forget pattern: lock, call `vm.sys_remove_link()`, flip span.
  - Acceptance: method calls `sys_remove_link` on the VM

#### [2.4] Modify `send()` to handle linked flag
- [ ] **2.4.1** In `src/endpoint/context.rs`, modify `ContextInternal::send()` to accept a `linked: bool` parameter. When `true`, after the `sys_send()` call, extract `name`/`key` from `RequestTarget` and call `create_link()`.
  - Acceptance: linked send emits both `sys_send` and `sys_create_link`
  - Acceptance: non-linked send unchanged
- [ ] **2.4.2** Update `Request::send()` in `src/context/request.rs` to pass `self.linked` to `ContextInternal::send()`.
  - Acceptance: `send()` propagates the linked flag

#### [2.5] Add `request_linked()` to ContextClient trait
- [ ] **2.5.1** In `src/context/mod.rs`, add `request_linked()` method to `ContextClient` trait alongside `request()`. Delegates to `Request::new_linked()`.
  - Acceptance: `request_linked()` available on all context types via blanket impl

#### [2.6] Generate `run_linked()` in macro
- [ ] **2.6.1** In `macros/src/generator.rs`, in `client_method_tokens()`, detect `ServiceType::Workflow` + handler with `HandlerType::Workflow` (non-shared). Emit `run_linked()` alongside `run()` that calls `self.ctx.request_linked(target, input)`.
  - Acceptance: workflow clients have `run_linked()` method
  - Acceptance: service/object clients do NOT have `run_linked()`

#### [2.7] Generate `unlink()` in macro
- [ ] **2.7.1** In `macros/src/generator.rs`, in `impl_client_tokens()`, add `unlink()` method for `ServiceType::Workflow` clients. Calls `self.ctx.remove_link(service_name, &self.key)`.
  - Acceptance: workflow clients have `unlink()` method
  - Acceptance: service/object clients do NOT have `unlink()`

### Phase 3: Green Tests

#### [3.1] Verify all tests pass
- [ ] **3.1.1** Run `cargo test` — compile test in `tests/service.rs` passes, test-services compiles.
  - Acceptance: all tests pass, no regressions
- [ ] **3.1.2** Run `cargo clippy` — no warnings on new code.
  - Acceptance: clean clippy output

## Success Criteria

- `ctx.workflow_client::<Child>("key").run_linked(req).send()` compiles and emits both commands
- `ctx.workflow_client::<Child>("key").unlink()` compiles and emits RemoveLinkCommand
- `run_linked()` only exists on workflow clients, not service/object
- All existing tests pass

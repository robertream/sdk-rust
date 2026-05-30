//! Compile test: verify ObjectContextT (PromiseObject), WorkflowContextT, and promise_object API.

use restate_sdk::prelude::*;

// ---------------------------------------------------------------------------
// Promise object — ctx.resolve() must accept the Output type
// ---------------------------------------------------------------------------

#[restate_sdk::object(PromiseContextT)]
trait PromiseOrder {
    async fn create(input: String) -> HandlerResult<String>;
    async fn finalize() -> HandlerResult<()>;
    #[shared]
    async fn get_status() -> HandlerResult<String>;
}

// ---------------------------------------------------------------------------
// Promise object with default Output = ()
// ---------------------------------------------------------------------------

#[restate_sdk::object(PromiseContextT)]
trait UnitPromise {
    async fn resolve_it() -> HandlerResult<()>;
}

// ---------------------------------------------------------------------------
// Plain object — generates ObjectClientT for use from typed contexts
// ---------------------------------------------------------------------------

#[restate_sdk::object(ObjectContextT)]
trait ChildObject {
    async fn do_work(input: String) -> HandlerResult<String>;
}

// ---------------------------------------------------------------------------
// ObjectContextT — can link + on_completion, no resolve
// ---------------------------------------------------------------------------

#[restate_sdk::object(ObjectContextT)]
trait Agent {
    async fn run_task(input: String) -> HandlerResult<()>;
    // Result<Output, _> matches PromiseOrder::Output (String).
    async fn on_order_done(result: Result<String, TerminalError>) -> HandlerResult<()>;
    async fn get_status() -> HandlerResult<String>;
}

// ---------------------------------------------------------------------------
// WorkflowContextT — can link + handle.output() + handle.unlink() + start_linked + inv.unlink()
// ---------------------------------------------------------------------------

#[restate_sdk::workflow(WorkflowContextT)]
trait OrderWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
    async fn approve() -> HandlerResult<()>;
}

#[restate_sdk::workflow]
trait ChildWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
}

// ---------------------------------------------------------------------------
// Impls
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct PromiseOrderImpl;

#[allow(dead_code)]
struct UnitPromiseImpl;

#[allow(dead_code)]
struct ChildObjectImpl;

#[allow(dead_code)]
struct AgentImpl;
#[allow(dead_code)]
struct OrderWorkflowImpl;
#[allow(dead_code)]
struct ChildWorkflowImpl;

#[restate_sdk::promise(Output = String)]
impl PromiseOrder for PromiseOrderImpl {
    async fn create(&self, ctx: ObjectContextT<'_, Self>, input: String) -> HandlerResult<String> {
        ctx.set("input", input.clone());
        Ok(input)
    }
    async fn finalize(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        // ctx.resolve() is available because PromiseOrderImpl: PromiseObject
        ctx.resolve("order complete".to_string()).await?;
        Ok(())
    }
    async fn get_status(&self, _ctx: SharedObjectContext<'_>) -> HandlerResult<String> {
        Ok("ok".into())
    }
}

// Test 1: promise with default Output = ()
// #[restate_sdk::promise] with no Output param defaults to (), so ctx.resolve(()) must compile.
#[restate_sdk::promise]
impl UnitPromise for UnitPromiseImpl {
    async fn resolve_it(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        ctx.resolve(()).await?;
        Ok(())
    }
}

// Test 2: plain #[restate_sdk::object] generates ObjectClientT usable from typed contexts
impl ChildObject for ChildObjectImpl {
    async fn do_work(
        &self,
        _ctx: ObjectContextT<'_, Self>,
        input: String,
    ) -> HandlerResult<String> {
        Ok(input)
    }
}

#[restate_sdk::completion]
impl Agent for AgentImpl {
    async fn run_task(&self, ctx: ObjectContextT<'_, Self>, input: String) -> HandlerResult<()> {
        // Link with on_completion using promise_object builder
        ctx.promise_object::<PromiseOrderImpl>("order-123")
            .on_completion(Self::on_order_done)
            .link()
            .await?;

        // Test 5a: promise_object().unlink() on ObjectContextT
        ctx.promise_object::<PromiseOrderImpl>("order-unlink")
            .unlink()
            .await?;

        // start_linked with on_completion on a workflow from ObjectContextT
        ctx.workflow_client::<ChildWorkflowClient>("wf-key")
            .run("hello".to_string())
            .on_completion(Self::on_order_done)
            .start_linked()
            .await?;

        // Test 2: object_client using the macro-generated ObjectClientT
        let _client = ctx.object_client::<ChildObjectObjectClientT<Self>>("child-key");

        let _ = input;
        Ok(())
    }
    async fn on_order_done(
        &self,
        ctx: ObjectContextT<'_, Self>,
        result: Result<String, TerminalError>,
    ) -> HandlerResult<()> {
        if let Ok(summary) = result {
            ctx.set("last_result", summary);
        }
        Ok(())
    }
    async fn get_status(&self, _ctx: ObjectContextT<'_, Self>) -> HandlerResult<String> {
        Ok("ok".into())
    }
}

impl OrderWorkflow for OrderWorkflowImpl {
    async fn run(&self, ctx: WorkflowContextT<'_, Self>, input: String) -> HandlerResult<String> {
        // promise_object().link() returns LinkHandle<PromiseOrderImpl>
        let handle = ctx
            .promise_object::<PromiseOrderImpl>("order-456")
            .link()
            .await?;

        // output() — T::Output = String inferred from PromiseOrderImpl: PromiseObject
        let _: String = handle.output().await?;

        // Test 3: LinkHandle.unlink()
        handle.unlink().await?;

        // Test 5b: promise_object().unlink() on WorkflowContextT
        ctx.promise_object::<PromiseOrderImpl>("order-unlink-wf")
            .unlink()
            .await?;

        // Test 4: start_linked() + Invocation.unlink()
        let inv = ctx
            .workflow_client::<ChildWorkflowClient>("child-wf")
            .run(input.clone())
            .start_linked()
            .await?;
        inv.unlink().await?;

        Ok(input)
    }
    async fn approve(&self, _ctx: WorkflowContextT<'_, Self>) -> HandlerResult<()> {
        Ok(())
    }
}

impl ChildWorkflow for ChildWorkflowImpl {
    async fn run(&self, _ctx: WorkflowContext<'_>, input: String) -> HandlerResult<String> {
        Ok(input)
    }
}

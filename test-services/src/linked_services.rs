use super::linked_workflow::LinkedWorkflowChildClient;
use restate_sdk::prelude::*;

// ── Promise object (child) ────────────────────────────────────────────────────

#[restate_sdk::object(PromiseContextT)]
#[name = "LinkedPromiseOrder"]
pub(crate) trait LinkedPromiseOrder {
    #[name = "create"]
    async fn create(input: String) -> HandlerResult<String>;
    #[name = "finalize"]
    async fn finalize() -> HandlerResult<()>;
    #[shared]
    #[name = "getStatus"]
    async fn get_status() -> HandlerResult<String>;
}

pub(crate) struct LinkedPromiseOrderImpl;

const STATUS: &str = "status";
const RESULT: &str = "result";

#[restate_sdk::promise(Output = String)]
impl LinkedPromiseOrder for LinkedPromiseOrderImpl {
    async fn create(&self, ctx: ObjectContextT<'_, Self>, input: String) -> HandlerResult<String> {
        ctx.set(STATUS, "created".to_string());
        ctx.set(RESULT, input.clone());
        Ok(input)
    }

    async fn finalize(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        let result = ctx.get::<String>(RESULT).await?.unwrap_or_default();
        ctx.set(STATUS, "finalized".to_string());
        ctx.resolve(format!("done:{result}")).await?;
        Ok(())
    }

    async fn get_status(&self, ctx: SharedObjectContext<'_>) -> HandlerResult<String> {
        Ok(ctx
            .get::<String>(STATUS)
            .await?
            .unwrap_or_else(|| "unknown".to_string()))
    }
}

// ── Agent object (parent with on_completion) ─────────────────────────────────

#[restate_sdk::object(ObjectContextT)]
#[name = "LinkedAgent"]
pub(crate) trait LinkedAgent {
    #[name = "linkAndComplete"]
    async fn link_and_complete(input: String) -> HandlerResult<String>;
    #[name = "onOrderDone"]
    async fn on_order_done(result: Result<String, TerminalError>) -> HandlerResult<()>;
    #[name = "linkAndUnlink"]
    async fn link_and_unlink(input: String) -> HandlerResult<String>;
    #[name = "getResult"]
    async fn get_result() -> HandlerResult<String>;
    #[name = "linkWorkflow"]
    async fn link_workflow(input: String) -> HandlerResult<String>;
}

pub(crate) struct LinkedAgentImpl;

const AGENT_RESULT: &str = "agent_result";

#[restate_sdk::completion]
impl LinkedAgent for LinkedAgentImpl {
    async fn link_and_complete(
        &self,
        ctx: ObjectContextT<'_, Self>,
        input: String,
    ) -> HandlerResult<String> {
        // Link to the promise object with a completion handler
        ctx.promise_object::<LinkedPromiseOrderImpl>("order-1")
            .on_completion(Self::on_order_done)
            .link()
            .await?;

        // Trigger the child's create handler
        ctx.object_client::<LinkedPromiseOrderObjectClientT<Self>>("order-1")
            .create(input.clone())
            .send();

        Ok(input)
    }

    async fn on_order_done(
        &self,
        ctx: ObjectContextT<'_, Self>,
        result: Result<String, TerminalError>,
    ) -> HandlerResult<()> {
        let value = match result {
            Ok(v) => v,
            Err(e) => format!("error:{}", e.message()),
        };
        ctx.set(AGENT_RESULT, value);
        Ok(())
    }

    async fn link_and_unlink(
        &self,
        ctx: ObjectContextT<'_, Self>,
        input: String,
    ) -> HandlerResult<String> {
        // Link then immediately unlink
        ctx.promise_object::<LinkedPromiseOrderImpl>("order-2")
            .link()
            .await?;
        ctx.promise_object::<LinkedPromiseOrderImpl>("order-2")
            .unlink()
            .await?;
        Ok(input)
    }

    async fn get_result(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<String> {
        Ok(ctx
            .get::<String>(AGENT_RESULT)
            .await?
            .unwrap_or_else(|| "none".to_string()))
    }

    async fn link_workflow(
        &self,
        ctx: ObjectContextT<'_, Self>,
        input: String,
    ) -> HandlerResult<String> {
        ctx.workflow_client::<LinkedWorkflowChildClient>("wf-1")
            .run(input.clone())
            .on_completion(Self::on_order_done)
            .start_linked()
            .await?;
        Ok(input)
    }
}

// ── Orchestrator workflow (parent with handle.output()) ───────────────────────

#[restate_sdk::workflow(WorkflowContextT)]
#[name = "LinkedOrchestrator"]
pub(crate) trait LinkedOrchestrator {
    #[name = "run"]
    async fn run(input: String) -> HandlerResult<String>;
}

pub(crate) struct LinkedOrchestratorImpl;

impl LinkedOrchestrator for LinkedOrchestratorImpl {
    async fn run(&self, ctx: WorkflowContextT<'_, Self>, input: String) -> HandlerResult<String> {
        // Link to the promise object — returns a LinkHandle for awaiting output
        let handle = ctx
            .promise_object::<LinkedPromiseOrderImpl>("order-3")
            .link()
            .await?;

        // Drive the child: create then finalize
        ctx.object_client::<LinkedPromiseOrderObjectClientT<Self>>("order-3")
            .create(input.clone())
            .send();
        ctx.object_client::<LinkedPromiseOrderObjectClientT<Self>>("order-3")
            .finalize()
            .send();

        // Await the promise object's output
        let result = handle.output().await?;
        Ok(result)
    }
}

// ── Workflow that tests start_linked + Invocation methods ────────────────────

#[restate_sdk::workflow(WorkflowContextT)]
#[name = "LinkedWorkflowTest"]
pub(crate) trait LinkedWorkflowTest {
    #[name = "run"]
    async fn run(input: String) -> HandlerResult<String>;
}

pub(crate) struct LinkedWorkflowTestImpl;

impl LinkedWorkflowTest for LinkedWorkflowTestImpl {
    async fn run(&self, ctx: WorkflowContextT<'_, Self>, input: String) -> HandlerResult<String> {
        // Start a linked child workflow
        let inv = ctx
            .workflow_client::<LinkedWorkflowChildClient>("wf-test-child")
            .run(input.clone())
            .start_linked()
            .await?;

        // Verify the invocation has an ID
        let _id = inv.id().to_string();

        // Unlink — child continues independently
        inv.unlink().await?;

        Ok(input)
    }
}

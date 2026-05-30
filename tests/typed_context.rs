//! Compile test: verify ObjectContextT + StartRequestT with same-object enforcement.

use restate_sdk::prelude::*;

#[restate_sdk::object(ObjectContextT)]
trait TypedObject {
    async fn create_child(input: String) -> HandlerResult<()>;
    async fn handle_child_done(result: Result<String, TerminalError>) -> HandlerResult<()>;
}

#[restate_sdk::workflow]
trait TypedChildWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
}

#[allow(dead_code)]
struct TypedObjectImpl;

#[restate_sdk::completion]
impl TypedObject for TypedObjectImpl {
    async fn create_child(
        &self,
        ctx: ObjectContextT<'_, Self>,
        input: String,
    ) -> HandlerResult<()> {
        // ObjectContextT::workflow_client returns WorkflowClientT
        // WorkflowClientT::run() returns StartRequestT (not StartRequest)
        // on_completion enforces handler is on Self with matching Res
        let _fut = ctx
            .workflow_client::<TypedChildWorkflowClient>("key")
            .run(input)
            .on_completion(Self::handle_child_done)
            .start_linked();

        // start_linked without on_completion also works
        let _fut = ctx
            .workflow_client::<TypedChildWorkflowClient>("key")
            .run("other".to_string())
            .start_linked();

        Ok(())
    }

    async fn handle_child_done(
        &self,
        _ctx: ObjectContextT<'_, Self>,
        _result: Result<String, TerminalError>,
    ) -> HandlerResult<()> {
        Ok(())
    }
}

//! Compile test: verify on_completion works via ObjectContextT typed path.
//! Run with: cargo test --test completion_handler_prototype

use restate_sdk::prelude::*;

#[restate_sdk::object(ObjectContextT)]
trait MyObject {
    async fn create_child(input: String) -> HandlerResult<()>;
    async fn handle_child_done(result: Result<String, TerminalError>) -> HandlerResult<()>;
    async fn handle_i32_done(result: Result<i32, TerminalError>) -> HandlerResult<()>;
}

#[restate_sdk::workflow]
trait ChildWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
}

#[allow(dead_code)]
struct MyObjectImpl;

#[restate_sdk::completion]
impl MyObject for MyObjectImpl {
    async fn create_child(
        &self,
        ctx: ObjectContextT<'_, Self>,
        input: String,
    ) -> HandlerResult<()> {
        // on_completion with typed path — same-object enforced
        let _fut = ctx
            .workflow_client::<ChildWorkflowClient>("key")
            .run(input)
            .on_completion(Self::handle_child_done)
            .start_linked();

        // start_linked without on_completion still works
        let _fut = ctx
            .workflow_client::<ChildWorkflowClient>("key")
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

    async fn handle_i32_done(
        &self,
        _ctx: ObjectContextT<'_, Self>,
        _result: Result<i32, TerminalError>,
    ) -> HandlerResult<()> {
        Ok(())
    }
}

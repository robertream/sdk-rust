use restate_sdk::prelude::*;

#[restate_sdk::object(ObjectContextT)]
trait MyObject {
    async fn create_child(input: String) -> HandlerResult<()>;
    async fn handle_i32_done(result: Result<i32, TerminalError>) -> HandlerResult<()>;
}

#[restate_sdk::workflow]
trait ChildWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
}

struct MyObjectImpl;

#[restate_sdk::completion]
impl MyObject for MyObjectImpl {
    async fn create_child(&self, ctx: ObjectContextT<'_, Self>, input: String) -> HandlerResult<()> {
        // Child returns String, but handler expects Result<i32, TerminalError>
        ctx.workflow_client::<ChildWorkflowClient>("key")
            .run(input)
            .on_completion(Self::handle_i32_done)
            .start_linked();
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

fn main() {}

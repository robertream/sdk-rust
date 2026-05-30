use restate_sdk::prelude::*;

#[restate_sdk::object(ObjectContextT)]
trait MyObject {
    async fn create_child(input: String) -> HandlerResult<()>;
}

#[restate_sdk::workflow]
trait ChildWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
}

fn not_a_handler() {}

struct MyObjectImpl;

#[restate_sdk::completion]
impl MyObject for MyObjectImpl {
    async fn create_child(&self, ctx: ObjectContextT<'_, Self>, input: String) -> HandlerResult<()> {
        ctx.workflow_client::<ChildWorkflowClient>("key")
            .run(input)
            .on_completion(not_a_handler)
            .start_linked();
        Ok(())
    }
}

fn main() {}

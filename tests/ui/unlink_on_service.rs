use restate_sdk::prelude::*;

#[restate_sdk::service]
trait MyService {
    async fn my_handler(input: String) -> HandlerResult<String>;
}

#[restate_sdk::workflow]
trait MyWorkflow {
    async fn run(input: String) -> HandlerResult<String>;
}

struct MyWorkflowImpl;

impl MyWorkflow for MyWorkflowImpl {
    async fn run(&self, ctx: WorkflowContext<'_>, input: String) -> HandlerResult<String> {
        // unlink should NOT work with service clients (requires IntoWorkflowClient)
        ctx.unlink::<MyServiceClient>("key");

        Ok(input)
    }
}

fn main() {}

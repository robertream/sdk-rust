use restate_sdk::prelude::*;

#[restate_sdk::object(ObjectContextT)]
trait ObjectA {
    async fn create_child(input: String) -> HandlerResult<()>;
}

#[restate_sdk::object(ObjectContextT)]
trait ObjectB {
    async fn handle_done(result: Result<String, TerminalError>) -> HandlerResult<()>;
}

#[restate_sdk::workflow]
trait ChildWf {
    async fn run(input: String) -> HandlerResult<String>;
}

struct ObjectAImpl;

#[restate_sdk::completion]
impl ObjectA for ObjectAImpl {
    async fn create_child(
        &self,
        ctx: ObjectContextT<'_, Self>,
        input: String,
    ) -> HandlerResult<()> {
        // Should fail: ObjectBImpl::handle_done is on a different object
        ctx.workflow_client::<ChildWfClient>("key")
            .run(input)
            .on_completion(ObjectBImpl::handle_done)
            .start_linked();
        Ok(())
    }
}

struct ObjectBImpl;

impl ObjectB for ObjectBImpl {
    async fn handle_done(
        &self,
        _ctx: ObjectContextT<'_, Self>,
        _result: Result<String, TerminalError>,
    ) -> HandlerResult<()> {
        Ok(())
    }
}

fn main() {}

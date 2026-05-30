use restate_sdk::prelude::*;

#[restate_sdk::object(PromiseContextT)]
trait MyOrder {
    async fn create() -> HandlerResult<()>;
}

#[restate_sdk::object(ObjectContextT)]
trait MyAgent {
    async fn do_work() -> HandlerResult<()>;
    // Handler expects u32 but MyOrder::Output = String
    async fn on_done(result: Result<u32, TerminalError>) -> HandlerResult<()>;
}

struct MyOrderImpl;

#[restate_sdk::promise(Output = String)]
impl MyOrder for MyOrderImpl {
    async fn create(&self, _ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        Ok(())
    }
}

struct MyAgentImpl;

#[restate_sdk::completion]
impl MyAgent for MyAgentImpl {
    async fn do_work(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        // Should fail: handler expects u32 but Output = String
        ctx.promise_object::<MyOrderImpl>("key")
            .on_completion(Self::on_done)
            .link()
            .await?;
        Ok(())
    }

    async fn on_done(
        &self,
        _ctx: ObjectContextT<'_, Self>,
        _result: Result<u32, TerminalError>,
    ) -> HandlerResult<()> {
        Ok(())
    }
}

fn main() {}

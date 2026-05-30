use restate_sdk::prelude::*;

#[restate_sdk::object]
trait NotAPromise {
    async fn handler() -> HandlerResult<()>;
}

#[restate_sdk::object(ObjectContextT)]
trait Parent {
    async fn link_it() -> HandlerResult<()>;
}

struct ParentImpl;

#[restate_sdk::completion]
impl Parent for ParentImpl {
    async fn link_it(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        struct FakeImpl;
        // FakeImpl doesn't implement LinkedObject
        ctx.promise_object::<FakeImpl>("key").link().await?;
        Ok(())
    }
}

fn main() {}

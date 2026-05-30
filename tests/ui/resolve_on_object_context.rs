use restate_sdk::prelude::*;

#[restate_sdk::object(ObjectContextT)]
pub trait MyObject {
    async fn handler() -> HandlerResult<()>;
}

struct MyObjectImpl;

impl MyObject for MyObjectImpl {
    async fn handler(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        ctx.resolve(42u32).await?;
        Ok(())
    }
}

fn main() {}

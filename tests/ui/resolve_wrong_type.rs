use restate_sdk::prelude::*;

#[restate_sdk::object(PromiseContextT)]
pub trait Order {
    async fn finalize() -> HandlerResult<()>;
}

struct OrderImpl;

#[restate_sdk::promise(Output = String)]
impl Order for OrderImpl {
    async fn finalize(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        // Should fail: resolving with u32 when Output = String
        ctx.resolve(42u32).await?;
        Ok(())
    }
}

fn main() {}

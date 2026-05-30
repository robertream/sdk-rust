use restate_sdk::prelude::*;

#[restate_sdk::object(PromiseContextT)]
#[name = "PromiseOrder"]
pub(crate) trait PromiseOrder {
    #[name = "create"]
    async fn create(input: String) -> HandlerResult<()>;
    #[name = "finalize"]
    async fn finalize() -> HandlerResult<()>;
    #[shared]
    #[name = "getStatus"]
    async fn get_status() -> HandlerResult<String>;
}

pub(crate) struct PromiseOrderImpl;

const STATUS: &str = "status";
const RESULT: &str = "result";

#[restate_sdk::promise(Output = String)]
impl PromiseOrder for PromiseOrderImpl {
    async fn create(&self, ctx: ObjectContextT<'_, Self>, input: String) -> HandlerResult<()> {
        ctx.set(STATUS, "created".to_string());
        ctx.set(RESULT, input);
        Ok(())
    }

    async fn finalize(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        let result = ctx.get::<String>(RESULT).await?.unwrap_or_default();
        ctx.set(STATUS, "finalized".to_string());
        ctx.resolve(result).await?;
        Ok(())
    }

    async fn get_status(&self, ctx: SharedObjectContext<'_>) -> HandlerResult<String> {
        Ok(ctx
            .get::<String>(STATUS)
            .await?
            .unwrap_or("unknown".to_string()))
    }
}

//! Example: Linked Services — Promise Objects, Typed Contexts, and Completion Handlers
//!
//! Demonstrates the full linked workflow API surface:
//!
//! - `#[restate_sdk::workflow(WorkflowContextT)]` — workflow that can link to children
//! - `#[restate_sdk::object(PromiseContextT)]` — promise object that can resolve
//! - `#[restate_sdk::object(ObjectContextT)]` — object with typed completion handlers
//! - `ctx.promise_object::<T>(key)` — builder for linking to a promise object
//! - `LinkHandle<T>` — typed handle with `.output()` for awaiting the result
//! - `ctx.resolve(result)` — resolve promise object back to parent
//! - `on_completion(Self::handler)` — typed completion handler with same-object enforcement

use restate_sdk::prelude::*;

// ---------------------------------------------------------------------------
// 1. Promise Object — processes input and resolves with a result
// ---------------------------------------------------------------------------

/// A promise object that does work and resolves back to its parent.
/// Declared with `PromiseContextT` attribute — exclusive handlers get `ObjectContextT` with `ctx.resolve()`.
#[restate_sdk::object(PromiseContextT)]
pub trait Order {
    /// Creates the order.
    async fn create(input: String) -> HandlerResult<String>;

    /// Finalizes the order and resolves back to the parent.
    async fn finalize() -> HandlerResult<()>;

    /// Anyone can query the current status.
    #[shared]
    async fn get_status() -> HandlerResult<String>;
}

pub struct OrderImpl;

#[restate_sdk::promise(Output = String)]
impl Order for OrderImpl {
    async fn create(&self, ctx: ObjectContextT<'_, Self>, input: String) -> HandlerResult<String> {
        // Store the input for later finalization
        ctx.set("input", input);
        ctx.set("status", "created".to_string());
        Ok("created".to_string())
    }

    async fn finalize(&self, ctx: ObjectContextT<'_, Self>) -> HandlerResult<()> {
        let input: String = ctx.get("input").await?.unwrap_or_default();
        let result = format!("processed: {input}");

        // Resolve — delivers the result to the parent.
        // After this, state becomes immutable.
        ctx.resolve(result).await?;
        ctx.set("status", "resolved".to_string());
        Ok(())
    }

    async fn get_status(&self, ctx: SharedObjectContext<'_>) -> HandlerResult<String> {
        Ok(format!("order:{}", ctx.key()))
    }
}

// ---------------------------------------------------------------------------
// 2. Agent — uses ObjectContextT with on_completion handler
// ---------------------------------------------------------------------------

/// An agent that links to a child and receives results via a completion handler.
/// Uses `ObjectContextT` for typed `on_completion` enforcement.
#[restate_sdk::object(ObjectContextT)]
pub trait Agent {
    async fn run_task(input: String) -> HandlerResult<()>;

    /// Called automatically when the linked child resolves.
    async fn on_order_done(result: Result<String, TerminalError>) -> HandlerResult<()>;
}

pub struct AgentImpl;

#[restate_sdk::completion]
impl Agent for AgentImpl {
    async fn run_task(&self, ctx: ObjectContextT<'_, Self>, _input: String) -> HandlerResult<()> {
        // Link to a promise object with a typed completion handler.
        // on_order_done is called on THIS object when the order resolves.
        ctx.promise_object::<OrderImpl>("order-123")
            .on_completion(Self::on_order_done)
            .link()
            .await?;

        Ok(())
    }

    async fn on_order_done(
        &self,
        ctx: ObjectContextT<'_, Self>,
        result: Result<String, TerminalError>,
    ) -> HandlerResult<()> {
        match result {
            Ok(summary) => ctx.set("last_result", summary),
            Err(e) => ctx.set("last_error", e.to_string()),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 3. Orchestrator — uses WorkflowContextT to link and directly await results
// ---------------------------------------------------------------------------

/// A workflow that links to children and awaits their results inline.
/// Uses `WorkflowContextT` — the only context with `promise_object().link()` returning `LinkHandle`.
#[restate_sdk::workflow(WorkflowContextT)]
pub trait Orchestrator {
    async fn run(input: String) -> HandlerResult<String>;
}

pub struct OrchestratorImpl;

impl Orchestrator for OrchestratorImpl {
    async fn run(&self, ctx: WorkflowContextT<'_, Self>, _input: String) -> HandlerResult<String> {
        // Link to a promise object — returns a LinkHandle<OrderImpl>
        let handle = ctx.promise_object::<OrderImpl>("order-789").link().await?;

        // output() returns a DurableFuture compatible with restate_sdk::select!
        // T::Output = String is inferred from OrderImpl: PromiseObject
        let result: String = handle.output().await?;

        Ok(format!("workflow done: {result}"))
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    HttpServer::new(
        Endpoint::builder()
            .bind(OrderImpl.serve())
            .bind(AgentImpl.serve())
            .bind(OrchestratorImpl.serve())
            .build(),
    )
    .listen_and_serve("0.0.0.0:9080".parse().unwrap())
    .await;
}

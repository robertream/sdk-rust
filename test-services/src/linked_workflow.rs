use restate_sdk::prelude::*;

#[restate_sdk::workflow]
#[name = "LinkedWorkflowChild"]
pub(crate) trait LinkedWorkflowChild {
    #[name = "run"]
    async fn run(input: String) -> HandlerResult<String>;
}

pub(crate) struct LinkedWorkflowChildImpl;

impl LinkedWorkflowChild for LinkedWorkflowChildImpl {
    async fn run(&self, _context: WorkflowContext<'_>, input: String) -> HandlerResult<String> {
        Ok(input)
    }
}

#[restate_sdk::workflow]
#[name = "LinkedWorkflowParent"]
pub(crate) trait LinkedWorkflowParent {
    #[name = "run"]
    async fn run(input: String) -> HandlerResult<String>;
}

pub(crate) struct LinkedWorkflowParentImpl;

impl LinkedWorkflowParent for LinkedWorkflowParentImpl {
    async fn run(&self, context: WorkflowContext<'_>, input: String) -> HandlerResult<String> {
        // Start child workflow with a link — parent won't complete until child does
        let _fut = context
            .workflow_client::<LinkedWorkflowChildClient>("child-1")
            .run(input.clone())
            .start_linked();

        // Remove the link — child continues independently, parent can complete
        context
            .unlink::<LinkedWorkflowChildClient>("child-1")
            .await?;

        Ok(input)
    }
}

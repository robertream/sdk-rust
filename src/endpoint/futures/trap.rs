use crate::context::InvocationHandle;
use crate::errors::TerminalError;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future that traps the execution at this point, but keeps waking up the waker
pub struct TrapFuture<T>(PhantomData<fn() -> T>);

impl<T> Default for TrapFuture<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> Future for TrapFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<T> {
        ctx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl<T> crate::context::macro_support::SealedDurableFuture for TrapFuture<T> {
    fn inner_context(&self) -> crate::endpoint::ContextInternal {
        unreachable!("TrapFuture should never be polled for inner_context")
    }

    fn handle(&self) -> restate_sdk_shared_core::NotificationHandle {
        unreachable!("TrapFuture should never be polled for handle")
    }
}

impl<T> crate::context::DurableFuture for TrapFuture<T> {}

impl<T> InvocationHandle for TrapFuture<T> {
    fn invocation_id(&self) -> impl Future<Output = Result<String, TerminalError>> + Send {
        TrapFuture::default()
    }

    fn cancel(&self) -> impl Future<Output = Result<(), TerminalError>> + Send {
        TrapFuture::default()
    }

    fn resolve(
        &self,
    ) -> impl Future<Output = Result<crate::endpoint::Invocation, TerminalError>> + Send {
        TrapFuture::default()
    }
}

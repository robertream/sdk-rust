use super::DurableFuture;

use crate::endpoint::ContextInternal;
pub use crate::endpoint::Invocation;
use crate::errors::TerminalError;
use crate::serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::time::Duration;

/// Target of a request to a Restate service.
#[derive(Debug, Clone)]
pub enum RequestTarget {
    Service {
        name: String,
        handler: String,
    },
    Object {
        name: String,
        key: String,
        handler: String,
    },
    Workflow {
        name: String,
        key: String,
        handler: String,
    },
}

impl RequestTarget {
    pub fn service(name: impl Into<String>, handler: impl Into<String>) -> Self {
        Self::Service {
            name: name.into(),
            handler: handler.into(),
        }
    }

    pub fn object(
        name: impl Into<String>,
        key: impl Into<String>,
        handler: impl Into<String>,
    ) -> Self {
        Self::Object {
            name: name.into(),
            key: key.into(),
            handler: handler.into(),
        }
    }

    pub fn workflow(
        name: impl Into<String>,
        key: impl Into<String>,
        handler: impl Into<String>,
    ) -> Self {
        Self::Workflow {
            name: name.into(),
            key: key.into(),
            handler: handler.into(),
        }
    }
}

impl fmt::Display for RequestTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestTarget::Service { name, handler } => write!(f, "{name}/{handler}"),
            RequestTarget::Object { name, key, handler } => write!(f, "{name}/{key}/{handler}"),
            RequestTarget::Workflow { name, key, handler } => write!(f, "{name}/{key}/{handler}"),
        }
    }
}

/// This struct encapsulates the parameters for a request to a service.
pub struct Request<'a, Req, Res = ()> {
    ctx: &'a ContextInternal,
    request_target: RequestTarget,
    idempotency_key: Option<String>,
    headers: Vec<(String, String)>,
    req: Req,
    res: PhantomData<Res>,
}

impl<'a, Req, Res> Request<'a, Req, Res> {
    pub(crate) fn new(ctx: &'a ContextInternal, request_target: RequestTarget, req: Req) -> Self {
        Self {
            ctx,
            request_target,
            idempotency_key: None,
            headers: vec![],
            req,
            res: PhantomData,
        }
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.headers.push((key, value));
        self
    }

    /// Add idempotency key to the request
    pub fn idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    /// Call a service. This returns a future encapsulating the response.
    pub fn call(self) -> impl CallFuture<Response = Res> + Send
    where
        Req: Serialize + 'static,
        Res: Deserialize + 'static,
    {
        self.ctx.call(
            self.request_target,
            self.idempotency_key,
            self.headers,
            self.req,
        )
    }

    /// Send the request to the service, without waiting for the response.
    pub fn send(self) -> impl InvocationHandle
    where
        Req: Serialize + 'static,
    {
        self.ctx.send(
            self.request_target,
            self.idempotency_key,
            self.headers,
            self.req,
            None,
        )
    }

    /// Schedule the request to the service, without waiting for the response.
    pub fn send_after(self, delay: Duration) -> impl InvocationHandle
    where
        Req: Serialize + 'static,
    {
        self.ctx.send(
            self.request_target,
            self.idempotency_key,
            self.headers,
            self.req,
            Some(delay),
        )
    }
}

/// A request for a workflow `run` handler. Supports all standard request methods
/// plus [`start_linked`](StartRequest::start_linked) for creating linked workflow invocations.
pub struct StartRequest<'a, Req, Res = ()> {
    inner: Request<'a, Req, Res>,
}

impl<'a, Req, Res> StartRequest<'a, Req, Res> {
    pub(crate) fn new(ctx: &'a ContextInternal, request_target: RequestTarget, req: Req) -> Self {
        Self {
            inner: Request::new(ctx, request_target, req),
        }
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.inner = self.inner.header(key, value);
        self
    }

    /// Add idempotency key to the request
    pub fn idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.inner = self.inner.idempotency_key(idempotency_key);
        self
    }

    /// Call the workflow run handler and wait for the response.
    pub fn call(self) -> impl CallFuture<Response = Res> + Send
    where
        Req: Serialize + 'static,
        Res: Deserialize + 'static,
    {
        self.inner.call()
    }

    /// Start the workflow without waiting for the response.
    pub fn send(self) -> impl InvocationHandle
    where
        Req: Serialize + 'static,
    {
        self.inner.send()
    }

    /// Schedule the workflow to start after a delay.
    pub fn send_after(self, delay: Duration) -> impl InvocationHandle
    where
        Req: Serialize + 'static,
    {
        self.inner.send_after(delay)
    }

    /// Start the workflow and create a link from the current invocation.
    /// Returns an [`Invocation<Res>`] with `id()` and `cancel()`.
    /// Use [`LinkHandle::output`] to await the child's completion.
    pub fn start_linked(self) -> impl Future<Output = Result<Invocation<Res>, TerminalError>> + Send
    where
        Req: Serialize + 'static,
    {
        self.inner.ctx.link(
            self.inner.request_target,
            self.inner.idempotency_key,
            self.inner.headers,
            self.inner.req,
            None,
        )
    }
}

/// A linked workflow request with a completion handler. The only terminal method
/// is [`start_linked`](LinkedRunRequest::start_linked).
pub struct LinkedRunRequest<'a, Req, Res = ()> {
    inner: Request<'a, Req, Res>,
    completion_handler_name: String,
}

impl<'a, Req, Res> LinkedRunRequest<'a, Req, Res> {
    /// Start the workflow, create a link, and register the completion handler.
    /// Returns an [`Invocation`] for the linked child.
    pub fn start_linked(self) -> impl Future<Output = Result<Invocation<Res>, TerminalError>> + Send
    where
        Req: Serialize + 'static,
    {
        self.inner.ctx.link(
            self.inner.request_target,
            self.inner.idempotency_key,
            self.inner.headers,
            self.inner.req,
            Some(self.completion_handler_name),
        )
    }
}

/// Marker trait for completion handlers with same-object enforcement.
///
/// Uses [`ObjectContextT<'ctx, S>`](crate::context::ObjectContextT) in the `AsyncFn` bound
/// so that `S` must match both the handler's receiver and the calling context.
pub trait CompletionHandlerFnT<S, Res> {}

impl<S, Res, F> CompletionHandlerFnT<S, Res> for F where
    F: for<'a, 'ctx> AsyncFn(
        &'a S,
        crate::context::ObjectContextT<'ctx, S>,
        Result<Res, TerminalError>,
    ) -> crate::errors::HandlerResult<()>
{
}

/// A typed request for a workflow `run` handler, carrying the parent service type `S`.
///
/// Created when calling `.run()` on a workflow client from an [`ObjectContextT<'ctx, S>`](crate::context::ObjectContextT).
/// Provides [`on_completion`](StartRequestT::on_completion) with compile-time same-object enforcement.
pub struct StartRequestT<'a, Req, Res, S> {
    inner: Request<'a, Req, Res>,
    _parent: PhantomData<S>,
}

impl<'a, Req, Res, S> StartRequestT<'a, Req, Res, S> {
    #[doc(hidden)]
    pub fn new(ctx: &'a ContextInternal, request_target: RequestTarget, req: Req) -> Self {
        Self {
            inner: Request::new(ctx, request_target, req),
            _parent: PhantomData,
        }
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.inner = self.inner.header(key, value);
        self
    }

    /// Add idempotency key to the request
    pub fn idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.inner = self.inner.idempotency_key(idempotency_key);
        self
    }

    /// Call the workflow run handler and wait for the response.
    pub fn call(self) -> impl CallFuture<Response = Res> + Send
    where
        Req: Serialize + 'static,
        Res: Deserialize + 'static,
    {
        self.inner.call()
    }

    /// Start the workflow without waiting for the response.
    pub fn send(self) -> impl InvocationHandle
    where
        Req: Serialize + 'static,
    {
        self.inner.send()
    }

    /// Schedule the workflow to start after a delay.
    pub fn send_after(self, delay: Duration) -> impl InvocationHandle
    where
        Req: Serialize + 'static,
    {
        self.inner.send_after(delay)
    }

    /// Start the workflow and create a link from the current invocation.
    /// Returns an [`Invocation<Res>`] with `id()` and `cancel()`.
    /// Use [`LinkHandle::output`] to await the child's completion.
    pub fn start_linked(self) -> impl Future<Output = Result<Invocation<Res>, TerminalError>> + Send
    where
        Req: Serialize + 'static,
    {
        self.inner.ctx.link(
            self.inner.request_target,
            self.inner.idempotency_key,
            self.inner.headers,
            self.inner.req,
            None,
        )
    }

    /// Register a completion handler to be invoked when the linked child completes.
    ///
    /// The handler must be an async method on the **same** virtual object (`S`) with signature:
    /// `async fn(&self, ObjectContextT<'_, Self>, Result<Res, TerminalError>) -> HandlerResult<()>`
    ///
    /// Compile-time enforced: `S` must match both the calling context and the handler's receiver.
    pub fn on_completion<F>(self, _handler: F) -> LinkedRunRequest<'a, Req, Res>
    where
        F: CompletionHandlerFnT<S, Res>,
        S: super::handler::NamedService,
    {
        let completion_handler_name =
            <S::Resolution as super::handler::NameResolution<S>>::resolve::<Res, F>().to_string();
        LinkedRunRequest {
            inner: self.inner,
            completion_handler_name,
        }
    }
}

/// Builder for linking to a promise object from an `ObjectContextT`.
/// Supports `.on_completion()` and `.link()`/`.unlink()`.
pub struct PromiseObjectRefT<'a, T: super::LinkedObject, S> {
    ctx: &'a ContextInternal,
    key: String,
    _phantom: PhantomData<(T, S)>,
}

impl<'a, T: super::LinkedObject, S> PromiseObjectRefT<'a, T, S> {
    pub(crate) fn new(ctx: &'a ContextInternal, key: String) -> Self {
        Self {
            ctx,
            key,
            _phantom: PhantomData,
        }
    }

    /// Link to the promise object. Returns when the link is established.
    pub fn link(self) -> impl Future<Output = Result<(), TerminalError>> + Send {
        self.ctx.link_service(T::SERVICE_NAME, self.key, None)
    }

    /// Unlink from the promise object.
    pub fn unlink(self) -> impl Future<Output = Result<(), TerminalError>> + Send {
        self.ctx.unlink_service(T::SERVICE_NAME, self.key)
    }

    /// Register a completion handler to be invoked when the promise object resolves.
    /// The handler's result parameter must match `T::Output`.
    pub fn on_completion<F>(self, _handler: F) -> PromiseObjectRef<'a>
    where
        F: CompletionHandlerFnT<S, T::Output>,
        S: super::handler::NamedService,
    {
        let completion_handler_name =
            <S::Resolution as super::handler::NameResolution<S>>::resolve::<T::Output, F>()
                .to_string();
        PromiseObjectRef {
            ctx: self.ctx,
            service: T::SERVICE_NAME,
            key: self.key,
            completion_handler_name,
        }
    }
}

/// Builder for linking to a promise object from a `WorkflowContextT`.
/// `.link()` returns a `LinkHandle<T>` for awaiting the output.
pub struct PromiseObjectRefW<'a, T: super::LinkedObject> {
    ctx: &'a ContextInternal,
    key: String,
    _phantom: PhantomData<T>,
}

impl<'a, T: super::LinkedObject> PromiseObjectRefW<'a, T> {
    pub(crate) fn new(ctx: &'a ContextInternal, key: String) -> Self {
        Self {
            ctx,
            key,
            _phantom: PhantomData,
        }
    }

    /// Link to the promise object. Returns a `LinkHandle<T>` for awaiting the output.
    pub fn link(self) -> impl Future<Output = Result<LinkHandle<T>, TerminalError>> + Send {
        let ctx = self.ctx.clone();
        let service_name = T::SERVICE_NAME;
        let key = self.key;
        async move {
            ctx.link_service(service_name, key.clone(), None).await?;
            Ok(LinkHandle {
                ctx,
                service_name,
                key,
                _phantom: PhantomData,
            })
        }
    }

    /// Unlink from the promise object.
    pub fn unlink(self) -> impl Future<Output = Result<(), TerminalError>> + Send {
        self.ctx.unlink_service(T::SERVICE_NAME, self.key)
    }
}

/// A promise object link request with a completion handler attached.
/// The output type `T` has been resolved by `on_completion`, so this type
/// only carries the service name, key, and handler name.
pub struct PromiseObjectRef<'a> {
    ctx: &'a ContextInternal,
    service: &'static str,
    key: String,
    completion_handler_name: String,
}

impl PromiseObjectRef<'_> {
    /// Execute the link with the registered completion handler.
    pub fn link(self) -> impl Future<Output = Result<(), TerminalError>> + Send {
        self.ctx
            .link_service(self.service, self.key, Some(self.completion_handler_name))
    }
}

/// Handle to a linked promise object, returned from `WorkflowContextT::promise_object().link()`.
/// Use `.output()` to await the promise object's result as a `DurableFuture`.
pub struct LinkHandle<T: super::PromiseObject> {
    ctx: ContextInternal,
    service_name: &'static str,
    key: String,
    _phantom: PhantomData<T>,
}

impl<T: super::PromiseObject> LinkHandle<T> {
    /// Get the output of the linked promise object as a `DurableFuture`.
    /// The result type is inferred from `T::Output`. Works with `select!`.
    pub fn output(
        &self,
    ) -> impl super::DurableFuture<Output = Result<T::Output, TerminalError>> + Send
    where
        T::Output: crate::serde::Deserialize + 'static,
    {
        self.ctx
            .attach_service::<T::Output>(self.service_name, &self.key)
    }

    /// Unlink from the promise object. The child continues running independently.
    pub fn unlink(&self) -> impl Future<Output = Result<(), TerminalError>> + Send {
        self.ctx.unlink_service(self.service_name, self.key.clone())
    }
}

pub trait InvocationHandle {
    fn invocation_id(&self) -> impl Future<Output = Result<String, TerminalError>> + Send;
    fn cancel(&self) -> impl Future<Output = Result<(), TerminalError>> + Send;
    fn resolve(&self) -> impl Future<Output = Result<Invocation, TerminalError>> + Send;
}

pub trait CallFuture:
    DurableFuture<Output = Result<Self::Response, TerminalError>> + InvocationHandle
{
    type Response;
}

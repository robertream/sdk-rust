use crate::ast::{Handler, Object, Service, ServiceInner, ServiceType, Workflow};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Literal};
use quote::{ToTokens, format_ident, quote};

/// Which context type to use for exclusive (non-shared) handlers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContextOption {
    /// Default context (`ObjectContext` or `WorkflowContext`).
    Plain,
    /// `ObjectContextT<'_, Self>` — enables typed workflow/object client.
    ObjectContextT,
    /// `PromiseContextT<'_, Self>` — typed client + resolve capability.
    PromiseContextT,
    /// `WorkflowContextT<'_, Self>` — typed workflow/object client for workflows.
    WorkflowContextT,
}

pub(crate) struct ServiceGenerator<'a> {
    service: ServiceScope<'a>,
    handlers: Vec<HandlerScope<'a>>,
}

impl<'a> ServiceGenerator<'a> {
    pub(crate) fn new_service(s: &'a Service) -> Self {
        Self::new(ServiceType::Service, &s.0, ContextOption::Plain)
    }

    pub(crate) fn new_object_with_options(s: &'a Object, context_option: ContextOption) -> Self {
        Self::new(ServiceType::Object, &s.0, context_option)
    }

    pub(crate) fn new_workflow_with_options(
        s: &'a Workflow,
        context_option: ContextOption,
    ) -> Self {
        Self::new(ServiceType::Workflow, &s.0, context_option)
    }

    fn new(service_ty: ServiceType, s: &'a ServiceInner, context_option: ContextOption) -> Self {
        ServiceGenerator {
            service: ServiceScope::new(service_ty, s, context_option),
            handlers: s.handlers.iter().map(HandlerScope::from_handler).collect(),
        }
    }

    fn trait_service(&self) -> TokenStream2 {
        self.service.trait_service_tokens(self.handlers.iter())
    }

    fn struct_serve(&self) -> TokenStream2 {
        self.service.struct_serve_tokens()
    }

    fn impl_service_for_serve(&self) -> TokenStream2 {
        self.service
            .impl_service_for_serve_tokens(self.handlers.iter())
    }

    fn impl_discoverable(&self) -> TokenStream2 {
        self.service.impl_discoverable_tokens(self.handlers.iter())
    }

    fn struct_client(&self) -> TokenStream2 {
        self.service.struct_client_tokens()
    }

    fn impl_client(&self) -> TokenStream2 {
        self.service.impl_client_tokens(self.handlers.iter())
    }

    fn struct_ingress(&self) -> TokenStream2 {
        self.service.struct_ingress_tokens()
    }

    fn impl_ingress(&self) -> TokenStream2 {
        self.service.impl_ingress_tokens(self.handlers.iter())
    }

    fn impl_handler_name_resolution(&self) -> TokenStream2 {
        self.service
            .impl_handler_name_resolution_tokens(self.handlers.iter())
    }
}

impl ToTokens for ServiceGenerator<'_> {
    fn to_tokens(&self, output: &mut TokenStream2) {
        output.extend(vec![
            self.trait_service(),
            self.struct_serve(),
            self.impl_service_for_serve(),
            self.impl_discoverable(),
            self.struct_client(),
            self.impl_client(),
            self.struct_ingress(),
            self.impl_ingress(),
            self.impl_handler_name_resolution(),
        ]);
    }
}

struct ServiceScope<'a> {
    service_ty: ServiceType,
    service: &'a ServiceInner,
    service_literal: Literal,
    client_ident: Ident,
    object_client_t_ident: Ident,
    handler_resolution_ident: Ident,
    ingress_ident: Ident,
    serve_ident: Ident,
    context_option: ContextOption,
}

impl ServiceScope<'_> {
    fn new<'a>(
        service_ty: ServiceType,
        s: &'a ServiceInner,
        context_option: ContextOption,
    ) -> ServiceScope<'a> {
        let service_literal = Literal::string(&s.restate_name);
        let client_ident = format_ident!("{}Client", s.ident);
        let object_client_t_ident = format_ident!("{}ObjectClientT", s.ident);
        let handler_resolution_ident = format_ident!("{}HandlerNameResolution", s.ident);
        let ingress_ident = format_ident!("Ingress{}", s.ident);
        let serve_ident = format_ident!("Serve{}", s.ident);

        ServiceScope {
            service_ty,
            service: s,
            service_literal,
            client_ident,
            object_client_t_ident,
            handler_resolution_ident,
            ingress_ident,
            serve_ident,
            context_option,
        }
    }

    fn trait_service_tokens<'a>(
        &self,
        handlers: impl IntoIterator<Item = &'a HandlerScope<'a>>,
    ) -> TokenStream2 {
        let service_ident = &self.service.ident;
        let serve_ident = &self.serve_ident;
        let vis = &self.service.vis;
        let attrs = &self.service.attrs;
        let handler_fns = handlers
            .into_iter()
            .map(|handler| handler.trait_method_tokens(self));

        quote! {
            #( #attrs )*
            #vis trait #service_ident: ::core::marker::Sized {
                #( #handler_fns )*

                /// Returns a serving function to use with [::restate_sdk::endpoint::Builder::with_service].
                fn serve(self) -> #serve_ident<Self> {
                    #serve_ident { service: ::std::sync::Arc::new(self) }
                }
            }
        }
    }

    fn struct_serve_tokens(&self) -> TokenStream2 {
        let vis = &self.service.vis;
        let serve_ident = &self.serve_ident;

        quote! {
            /// Struct implementing [::restate_sdk::service::Service], to be used with [::restate_sdk::endpoint::Builder::with_service].
            #[derive(Clone)]
            #vis struct #serve_ident<S> {
                service: ::std::sync::Arc<S>,
            }
        }
    }

    fn impl_service_for_serve_tokens<'a>(
        &self,
        handlers: impl IntoIterator<Item = &'a HandlerScope<'a>>,
    ) -> TokenStream2 {
        let serve_ident = &self.serve_ident;
        let service_ident = &self.service.ident;
        let match_arms = handlers.into_iter().map(|h| h.serve_match_arm_tokens());

        quote! {
            impl<S> ::restate_sdk::service::Service for #serve_ident<S>
                where S: #service_ident + Send + Sync + 'static,
            {
                type Future = ::restate_sdk::service::ServiceBoxFuture;

                fn handle(&self, ctx: ::restate_sdk::endpoint::ContextInternal) -> Self::Future {
                    let service_clone = ::std::sync::Arc::clone(&self.service);
                    ::std::boxed::Box::pin(async move {
                        match ctx.handler_name() {
                            #( #match_arms ),*
                            _ => {
                                return Err(::restate_sdk::endpoint::Error::unknown_handler(
                                    ctx.service_name(),
                                    ctx.handler_name(),
                                ))
                            }
                        }
                    })
                }
            }
        }
    }

    fn impl_discoverable_tokens<'a>(
        &self,
        handlers: impl IntoIterator<Item = &'a HandlerScope<'a>>,
    ) -> TokenStream2 {
        let serve_ident = &self.serve_ident;
        let service_ident = &self.service.ident;
        let service_literal = &self.service_literal;
        let service_ty_token = self.discovery_service_type_tokens();
        let handlers = handlers
            .into_iter()
            .map(|handler| handler.discovery_handler_tokens(self));

        quote! {
            impl<S> ::restate_sdk::service::Discoverable for #serve_ident<S>
                where S: #service_ident,
            {
                fn discover() -> ::restate_sdk::discovery::Service {
                    use  ::restate_sdk::discovery;
                    discovery::Service {
                        name: discovery::ServiceName::try_from(#service_literal).unwrap(),
                        ty: #service_ty_token,
                        handlers: vec![#( #handlers ),*],
                        documentation: None,
                        metadata: Default::default(),
                        abort_timeout: None,
                        inactivity_timeout: None,
                        journal_retention: None,
                        idempotency_retention: None,
                        enable_lazy_state: None,
                        ingress_private: None,
                        retry_policy_initial_interval: None,
                        retry_policy_max_interval: None,
                        retry_policy_max_attempts: None,
                        retry_policy_exponentiation_factor: None,
                        retry_policy_on_max_attempts: None,
                    }
                }
            }
        }
    }

    fn discovery_service_type_tokens(&self) -> TokenStream2 {
        match self.service_ty {
            ServiceType::Service => quote! { ::restate_sdk::discovery::ServiceType::Service },
            ServiceType::Object => quote! { ::restate_sdk::discovery::ServiceType::VirtualObject },
            ServiceType::Workflow => quote! { ::restate_sdk::discovery::ServiceType::Workflow },
        }
    }

    fn client_into_impl_tokens(&self) -> TokenStream2 {
        let client_ident = &self.client_ident;
        let service_literal = &self.service_literal;

        let service_client_impl = quote! {
            impl ::restate_sdk::context::ServiceClient for #client_ident<'_> {
                const SERVICE_NAME: &'static str = #service_literal;
            }
        };

        let specific_impl = match self.service_ty {
            ServiceType::Service => quote! {
                impl<'ctx> ::restate_sdk::context::IntoServiceClient<'ctx> for #client_ident<'ctx> {
                    fn create_client(ctx: &'ctx ::restate_sdk::endpoint::ContextInternal) -> Self {
                        Self { ctx }
                    }
                }
            },
            ServiceType::Object => quote! {
                impl<'ctx> ::restate_sdk::context::KeyedClient<'ctx> for #client_ident<'ctx> {
                    fn create_client(ctx: &'ctx ::restate_sdk::endpoint::ContextInternal, key: String) -> Self {
                        Self { ctx, key }
                    }
                }

                impl<'ctx> ::restate_sdk::context::IntoObjectClient<'ctx> for #client_ident<'ctx> {}
            },
            ServiceType::Workflow => quote! {
                impl<'ctx> ::restate_sdk::context::KeyedClient<'ctx> for #client_ident<'ctx> {
                    fn create_client(ctx: &'ctx ::restate_sdk::endpoint::ContextInternal, key: String) -> Self {
                        Self { ctx, key }
                    }
                }

                impl<'ctx> ::restate_sdk::context::IntoWorkflowClient<'ctx> for #client_ident<'ctx> {}
            },
        };

        quote! {
            #service_client_impl
            #specific_impl
        }
    }

    fn ingress_into_impl_tokens(&self) -> TokenStream2 {
        let client_ident = &self.client_ident;
        let ingress_ident = &self.ingress_ident;
        match self.service_ty {
            ServiceType::Service => quote! {
                impl<'ctx> ::restate_sdk::ingress::builder::IntoServiceRequest for #client_ident<'ctx> {
                    type Request<'a> = #ingress_ident<'a>;

                    fn create_request<'a>(client: &'a ::restate_sdk::ingress::Client) -> Self::Request<'a> {
                        #ingress_ident {
                            client,
                        }
                    }
                }
            },
            ServiceType::Object => quote! {
                impl<'ctx> ::restate_sdk::ingress::builder::IntoObjectRequest for #client_ident<'ctx> {
                    type Request<'a> = #ingress_ident<'a>;

                    fn create_request<'a>(client: &'a ::restate_sdk::ingress::Client, key: String) -> Self::Request<'a> {
                        #ingress_ident {
                            client,
                            key,
                        }
                    }
                }
            },
            ServiceType::Workflow => quote! {
                impl<'ctx> ::restate_sdk::ingress::builder::IntoWorkflowRequest for #client_ident<'ctx> {
                    type Request<'a> = #ingress_ident<'a>;

                    fn create_request<'a>(client: &'a ::restate_sdk::ingress::Client, key: String) -> Self::Request<'a> {
                        #ingress_ident {
                            client,
                            key,
                        }
                    }
                }
            },
        }
    }

    fn struct_ingress_tokens(&self) -> TokenStream2 {
        let vis = &self.service.vis;
        let ingress_ident = &self.ingress_ident;
        let key_field = self.key_field_tokens();
        let ingress_into_client_impl = self.ingress_into_impl_tokens();
        let service_ident = &self.service.ident;
        let doc_msg = format!(
            "Struct exposing the ingress client to invoke [`{service_ident}`] from from the ingress API."
        );

        quote! {
            #[doc = #doc_msg]
            #vis struct #ingress_ident<'a> {
                client: &'a ::restate_sdk::ingress::Client,
                #key_field
            }

            #ingress_into_client_impl
        }
    }

    fn struct_client_tokens(&self) -> TokenStream2 {
        let vis = &self.service.vis;
        let client_ident = &self.client_ident;
        let key_field = self.key_field_tokens();
        let client_into_impl_tokens = self.client_into_impl_tokens();
        let service_ident = &self.service.ident;
        let doc_msg = format!(
            "Struct exposing the client to invoke [`{service_ident}`] from another service."
        );

        quote! {
            #[doc = #doc_msg]
            #vis struct #client_ident<'ctx> {
                ctx: &'ctx ::restate_sdk::endpoint::ContextInternal,
                #key_field
            }

            #client_into_impl_tokens
        }
    }

    fn key_field_tokens(&self) -> TokenStream2 {
        match self.service_ty {
            ServiceType::Service => quote! {},
            ServiceType::Object | ServiceType::Workflow => quote! { key: String, },
        }
    }

    fn impl_client_tokens<'a>(
        &self,
        handlers: impl IntoIterator<Item = &'a HandlerScope<'a>>,
    ) -> TokenStream2 {
        let client_ident = &self.client_ident;
        let handlers: Vec<_> = handlers.into_iter().collect();
        let handlers_fns = handlers.iter().map(|h| h.client_method_tokens(self));
        let service_ident = &self.service.ident;
        let doc_msg = format!(
            "Struct exposing the client to invoke [`{service_ident}`] from another service."
        );

        // For workflow types, generate StartT impl for the run handler.
        // This allows WorkflowClientT::run() to return StartRequestT.
        let typed_run_impl = if self.service_ty == ServiceType::Workflow {
            if let Some(run_handler) = handlers
                .iter()
                .find(|h| !h.handler.is_shared && h.handler.restate_name == "run")
            {
                let argument_ty = run_handler.argument_type_tokens();
                let response_ty = &run_handler.handler.output_ok;
                let request_target = run_handler.context_request_target_tokens(self);
                let input = run_handler.input_tokens();

                quote! {
                    impl<'ctx> ::restate_sdk::context::StartT<'ctx> for #client_ident<'ctx> {
                        type Req = #argument_ty;
                        type Res = #response_ty;

                        fn start_typed<S>(&self, req: Self::Req) -> ::restate_sdk::context::StartRequestT<'ctx, Self::Req, Self::Res, S> {
                            ::restate_sdk::context::StartRequestT::new(self.ctx, #request_target, #input)
                        }
                    }
                }
            } else {
                quote! {}
            }
        } else {
            quote! {}
        };

        // Generate typed ObjectClientT for objects/workflows that have #[start] handlers.
        // This wrapper overrides #[start] methods to return StartRequestT, enabling start_linked()
        // and on_completion() from T-suffix contexts.
        let object_client_t_impl = self.object_client_t_tokens(handlers.iter().copied());

        quote! {
            #[doc = #doc_msg]
            impl<'ctx> #client_ident<'ctx> {
                #( #handlers_fns )*
            }

            #typed_run_impl
            #object_client_t_impl
        }
    }

    /// Generates the `{Service}ObjectClientT<'ctx, S>` wrapper struct and its impl.
    ///
    /// Emitted for all object and workflow service types. The wrapper:
    /// - Stores the inner client and a `PhantomData<S>` for the parent service type.
    /// - Implements `Deref` to the inner client to expose all regular handler methods.
    /// - For workflows, overrides the `run` handler to return `StartRequestT<'ctx, Req, Res, S>`.
    /// - Implements `IntoObjectClientT<'ctx, S>` so T-suffix contexts can create it.
    fn object_client_t_tokens<'a>(
        &self,
        handlers: impl IntoIterator<Item = &'a HandlerScope<'a>>,
    ) -> TokenStream2 {
        if self.service_ty == ServiceType::Service {
            return quote! {};
        }

        let handlers: Vec<_> = handlers.into_iter().collect();

        // Collect the non-shared start handlers (workflow `run` only).
        let start_handlers: Vec<_> = handlers
            .iter()
            .filter(|h| h.handler.is_start && !h.handler.is_shared)
            .collect();

        let vis = &self.service.vis;
        let client_ident = &self.client_ident;
        let object_client_t_ident = &self.object_client_t_ident;
        let service_ident = &self.service.ident;
        let doc_msg = format!(
            "Typed client wrapper for [`{service_ident}`]. \
             Implements `IntoObjectClientT` so it can be used from T-suffix contexts via `ctx.object_client()`."
        );

        // Generate override methods for start handlers (workflow `run`).
        let start_method_overrides = start_handlers.iter().map(|h| {
            let handler_ident = &h.handler.ident;
            let argument = h.argument_tokens();
            let argument_ty = h.argument_type_tokens();
            let response_ty = &h.handler.output_ok;
            let request_target = h.context_request_target_tokens_on_inner(self);
            let input = h.input_tokens();

            quote! {
                #vis fn #handler_ident(&self, #argument) -> ::restate_sdk::context::StartRequestT<'ctx, #argument_ty, #response_ty, S> {
                    ::restate_sdk::context::StartRequestT::new(self.inner.ctx, #request_target, #input)
                }
            }
        });

        quote! {
            #[doc = #doc_msg]
            #vis struct #object_client_t_ident<'ctx, S> {
                inner: #client_ident<'ctx>,
                _parent: ::std::marker::PhantomData<(&'ctx (), S)>,
            }

            impl<'ctx, S> ::std::ops::Deref for #object_client_t_ident<'ctx, S> {
                type Target = #client_ident<'ctx>;
                fn deref(&self) -> &Self::Target {
                    &self.inner
                }
            }

            impl<'ctx, S> #object_client_t_ident<'ctx, S> {
                #( #start_method_overrides )*
            }

            impl<'ctx, S> ::restate_sdk::context::IntoObjectClientT<'ctx, S> for #object_client_t_ident<'ctx, S> {
                fn create_typed_client(ctx: &'ctx ::restate_sdk::endpoint::ContextInternal, key: String) -> Self {
                    Self {
                        inner: #client_ident { ctx, key },
                        _parent: ::std::marker::PhantomData,
                    }
                }
            }
        }
    }

    /// Generate a per-service resolver struct and `NameResolution` impl.
    /// Only emitted for object types (which support `on_completion`).
    fn impl_handler_name_resolution_tokens<'a>(
        &self,
        handlers: impl IntoIterator<Item = &'a HandlerScope<'a>>,
    ) -> TokenStream2 {
        if self.service_ty != ServiceType::Object {
            return quote! {};
        }

        let vis = &self.service.vis;
        let service_ident = &self.service.ident;
        let handler_resolution_ident = &self.handler_resolution_ident;
        let resolver_arms = handlers.into_iter().map(|h| {
            let rust_name = h.handler.ident.to_string();
            let restate_name = &h.handler_literal;
            quote! { #rust_name => #restate_name }
        });

        quote! {
            #[doc(hidden)]
            #vis struct #handler_resolution_ident;

            impl<__T: #service_ident> ::restate_sdk::context::handler::NameResolution<__T> for #handler_resolution_ident {
                fn resolve<__Res, __F: ::restate_sdk::context::CompletionHandlerFnT<__T, __Res>>() -> &'static str {
                    let full = ::std::any::type_name::<__F>();
                    // type_name for function items always contains "::" — extract the method name.
                    // CompletionHandlerFnT<S, Res> guarantees F is a handler on S, and the match
                    // arms are generated from the same trait definition, so this is infallible.
                    match full.rsplit("::").next().unwrap() {
                        #( #resolver_arms, )*
                        method => unreachable!("handler '{method}' not in generated handler resolution"),
                    }
                }
            }
        }
    }

    fn impl_ingress_tokens<'a>(
        &self,
        handlers: impl IntoIterator<Item = &'a HandlerScope<'a>>,
    ) -> TokenStream2 {
        let ingress_ident = &self.ingress_ident;
        let handler_fns = handlers
            .into_iter()
            .map(|handler| handler.ingress_method_tokens(self));

        quote! {
            impl #ingress_ident<'_> {
                #( #handler_fns )*
            }
        }
    }
}

struct HandlerScope<'a> {
    handler: &'a Handler,
    arg_ty: Option<&'a syn::Type>,
    handler_literal: Literal,
}

impl HandlerScope<'_> {
    fn from_handler(handler: &Handler) -> HandlerScope<'_> {
        let arg_ty = handler.arg.as_ref().map(|pat_ty| pat_ty.ty.as_ref());
        let handler_literal = Literal::string(&handler.restate_name);

        HandlerScope {
            handler,
            arg_ty,
            handler_literal,
        }
    }

    fn trait_method_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        let attrs = &self.handler.attrs;
        let ident = &self.handler.ident;
        let ctx = self.handler_client_tokens(service);
        let args = self.handler.arg.iter();
        let output_ok = &self.handler.output_ok;
        let output_err = &self.handler.output_err;

        quote! {
            #( #attrs )*
            fn #ident(&self, context: #ctx, #( #args ),*) -> impl std::future::Future<Output=Result<#output_ok, #output_err>> + ::core::marker::Send;
        }
    }

    fn handler_client_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        match (service.service_ty, self.handler.is_shared) {
            (ServiceType::Service, _) => quote! { ::restate_sdk::prelude::Context },
            (ServiceType::Object, true) => quote! { ::restate_sdk::prelude::SharedObjectContext },
            (ServiceType::Object, false) => match service.context_option {
                ContextOption::ObjectContextT => {
                    quote! { ::restate_sdk::prelude::ObjectContextT<'_, Self> }
                }
                ContextOption::PromiseContextT => {
                    quote! { ::restate_sdk::prelude::ObjectContextT<'_, Self> }
                }
                _ => quote! { ::restate_sdk::prelude::ObjectContext },
            },
            (ServiceType::Workflow, true) => {
                quote! { ::restate_sdk::prelude::SharedWorkflowContext }
            }
            (ServiceType::Workflow, false) => match service.context_option {
                ContextOption::WorkflowContextT => {
                    quote! { ::restate_sdk::prelude::WorkflowContextT<'_, Self> }
                }
                _ => quote! { ::restate_sdk::prelude::WorkflowContext },
            },
        }
    }

    fn serve_match_arm_tokens(&self) -> TokenStream2 {
        let handler_ident = &self.handler.ident;
        let get_input_and_call = if self.arg_ty.is_some() {
            quote! {
                let (input, metadata) = ctx.input().await;
                let fut = S::#handler_ident(&service_clone, (&ctx, metadata).into(), input);
            }
        } else {
            quote! {
                let (_, metadata) = ctx.input::<()>().await;
                let fut = S::#handler_ident(&service_clone, (&ctx, metadata).into());
            }
        };
        let handler_literal = &self.handler_literal;

        quote! {
            #handler_literal => {
                #get_input_and_call
                let res = fut.await.map_err(::restate_sdk::errors::HandlerError::from);
                ctx.handle_handler_result(res);
                ctx.end();
                Ok(())
            }
        }
    }

    fn discovery_handler_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        let handler_literal = &self.handler_literal;
        let handler_ty = self.discovery_handler_type_tokens(service);
        let input_schema = self.discovery_input_schema_tokens();
        let output_schema = self.discovery_output_schema_tokens();
        let lazy_state = self.discovery_lazy_state_tokens();

        quote! {
            ::restate_sdk::discovery::Handler {
                name: ::restate_sdk::discovery::HandlerName::try_from(#handler_literal).expect("Handler name valid"),
                ty: #handler_ty,
                input: #input_schema,
                output: #output_schema,
                enable_lazy_state: #lazy_state,
                documentation: None,
                metadata: Default::default(),
                abort_timeout: None,
                inactivity_timeout: None,
                journal_retention: None,
                idempotency_retention: None,
                workflow_completion_retention: None,
                ingress_private: None,
                retry_policy_initial_interval: None,
                retry_policy_max_interval: None,
                retry_policy_max_attempts: None,
                retry_policy_exponentiation_factor: None,
                retry_policy_on_max_attempts: None,
            }
        }
    }

    fn discovery_handler_type_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        if self.handler.is_shared {
            quote! { Some(::restate_sdk::discovery::HandlerType::Shared) }
        } else if service.service_ty == ServiceType::Workflow {
            quote! { Some(::restate_sdk::discovery::HandlerType::Workflow) }
        } else {
            // Macro has same defaulting rules of the discovery manifest
            quote! { None }
        }
    }

    fn discovery_input_schema_tokens(&self) -> TokenStream2 {
        match self.arg_ty {
            Some(ty) => quote! {
                Some(::restate_sdk::discovery::InputPayload::from_metadata::<#ty>())
            },
            None => quote! {
                Some(::restate_sdk::discovery::InputPayload::empty())
            },
        }
    }

    fn discovery_output_schema_tokens(&self) -> TokenStream2 {
        let output_ty = &self.handler.output_ok;
        match output_ty {
            syn::Type::Tuple(tuple) if tuple.elems.is_empty() => quote! {
                Some(::restate_sdk::discovery::OutputPayload::empty())
            },
            _ => quote! {
                Some(::restate_sdk::discovery::OutputPayload::from_metadata::<#output_ty>())
            },
        }
    }

    fn discovery_lazy_state_tokens(&self) -> TokenStream2 {
        if self.handler.is_lazy_state {
            quote! { Some(true) }
        } else {
            quote! { None }
        }
    }

    fn client_method_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        let vis = &service.service.vis;
        let handler_ident = &self.handler.ident;
        let argument = self.argument_tokens();
        let argument_ty = self.argument_type_tokens();
        let response_ty = &self.handler.output_ok;
        let request_target = self.context_request_target_tokens(service);
        let input = self.input_tokens();

        // Workflow run handler returns StartRequest (which has .start_linked())
        let is_workflow_run = service.service_ty == ServiceType::Workflow
            && !self.handler.is_shared
            && self.handler.restate_name == "run";

        if is_workflow_run {
            quote! {
                #vis fn #handler_ident(&self, #argument) -> ::restate_sdk::context::StartRequest<'ctx, #argument_ty, #response_ty> {
                    self.ctx.start_request(#request_target, #input)
                }
            }
        } else {
            quote! {
                #vis fn #handler_ident(&self, #argument) -> ::restate_sdk::context::Request<'ctx, #argument_ty, #response_ty> {
                    self.ctx.request(#request_target, #input)
                }
            }
        }
    }

    fn ingress_method_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        let vis = &service.service.vis;
        let handler_ident = &self.handler.ident;
        let argument = self.argument_tokens();
        let response_ty = &self.handler.output_ok;
        let request_call = self.ingress_request_call_tokens(service);

        quote! {
            #vis fn #handler_ident(&self, #argument) -> ::restate_sdk::ingress::Request<#response_ty> {
                #request_call
            }
        }
    }

    fn context_request_target_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        let service_literal = &service.service_literal;
        let handler_literal = &self.handler_literal;
        match service.service_ty {
            ServiceType::Service => quote! {
                ::restate_sdk::context::RequestTarget::service(#service_literal, #handler_literal)
            },
            ServiceType::Object => quote! {
                ::restate_sdk::context::RequestTarget::object(#service_literal, &self.key, #handler_literal)
            },
            ServiceType::Workflow => quote! {
                ::restate_sdk::context::RequestTarget::workflow(#service_literal, &self.key, #handler_literal)
            },
        }
    }

    /// Like `context_request_target_tokens` but accesses the key via `self.inner.key`,
    /// for use in `ObjectClientT` methods that wrap an inner client.
    fn context_request_target_tokens_on_inner(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        let service_literal = &service.service_literal;
        let handler_literal = &self.handler_literal;
        match service.service_ty {
            ServiceType::Service => quote! {
                ::restate_sdk::context::RequestTarget::service(#service_literal, #handler_literal)
            },
            ServiceType::Object => quote! {
                ::restate_sdk::context::RequestTarget::object(#service_literal, &self.inner.key, #handler_literal)
            },
            ServiceType::Workflow => quote! {
                ::restate_sdk::context::RequestTarget::workflow(#service_literal, &self.inner.key, #handler_literal)
            },
        }
    }

    fn ingress_request_call_tokens(&self, service: &ServiceScope<'_>) -> TokenStream2 {
        let service_literal = &service.service_literal;
        let handler_literal = &self.handler_literal;
        let input = self.input_tokens();
        match service.service_ty {
            ServiceType::Service => quote! {
                ::restate_sdk::ingress::builder::service(self.client, concat!("/", #service_literal, "/", #handler_literal), #input)
            },
            ServiceType::Object => quote! {
                ::restate_sdk::ingress::builder::object(self.client, #service_literal, &self.key, #handler_literal, #input)
            },
            ServiceType::Workflow => quote! {
                ::restate_sdk::ingress::builder::workflow(self.client, #service_literal, &self.key, #handler_literal, #input)
            },
        }
    }

    fn argument_tokens(&self) -> TokenStream2 {
        match self.arg_ty {
            None => quote! {},
            Some(arg_ty) => quote! { req: #arg_ty },
        }
    }

    fn argument_type_tokens(&self) -> TokenStream2 {
        match self.arg_ty {
            None => quote! { () },
            Some(arg_ty) => quote! { #arg_ty },
        }
    }

    fn input_tokens(&self) -> TokenStream2 {
        if self.arg_ty.is_some() {
            quote! { req }
        } else {
            quote! { () }
        }
    }
}

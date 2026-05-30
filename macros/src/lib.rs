// Copyright (c) 2023 -  Restate Software, Inc., Restate GmbH.
// All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// Some parts of this codebase were taken from https://github.com/google/tarpc/blob/b826f332312d3702667880a464e247556ad7dbfe/plugins/src/lib.rs
// License MIT

extern crate proc_macro;

mod ast;
mod generator;

use crate::ast::{Object, Service, Workflow};
use crate::generator::{ContextOption, ServiceGenerator};
use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse_macro_input;

/// Parsed `Output = Type` from `#[restate_sdk::promise(Output = String)]`.
struct PromiseAttr {
    output_ty: syn::Type,
}

impl syn::parse::Parse for PromiseAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        if ident != "Output" {
            return Err(syn::Error::new_spanned(ident, "expected `Output`"));
        }
        let _: syn::Token![=] = input.parse()?;
        let output_ty: syn::Type = input.parse()?;
        Ok(Self { output_ty })
    }
}

#[proc_macro_attribute]
pub fn service(_: TokenStream, input: TokenStream) -> TokenStream {
    let svc = parse_macro_input!(input as Service);

    ServiceGenerator::new_service(&svc)
        .into_token_stream()
        .into()
}

#[proc_macro_attribute]
pub fn object(attr: TokenStream, input: TokenStream) -> TokenStream {
    let svc = parse_macro_input!(input as Object);
    let attr_str = attr.to_string();
    let attr_trimmed = attr_str.trim();

    let context_option = if attr.is_empty() {
        ContextOption::Plain
    } else if attr_trimmed == "ObjectContextT" {
        ContextOption::ObjectContextT
    } else if attr_trimmed == "PromiseContextT" {
        ContextOption::PromiseContextT
    } else {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "unknown object attribute `{attr_trimmed}`, expected `ObjectContextT` or `PromiseContextT`"
            ),
        )
        .to_compile_error()
        .into();
    };

    ServiceGenerator::new_object_with_options(&svc, context_option)
        .into_token_stream()
        .into()
}

#[proc_macro_attribute]
pub fn workflow(attr: TokenStream, input: TokenStream) -> TokenStream {
    let svc = parse_macro_input!(input as Workflow);
    let attr_str = attr.to_string();
    let attr_trimmed = attr_str.trim();

    let context_option = if attr.is_empty() {
        ContextOption::Plain
    } else if attr_trimmed == "WorkflowContextT" {
        ContextOption::WorkflowContextT
    } else {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("unknown workflow attribute `{attr_trimmed}`, expected `WorkflowContextT`"),
        )
        .to_compile_error()
        .into();
    };

    ServiceGenerator::new_workflow_with_options(&svc, context_option)
        .into_token_stream()
        .into()
}

/// Attribute macro for promise object implementations. Generates `PromiseObject`
/// and `LinkedObject` impls, connecting the concrete type to its output type
/// and service name.
///
/// ```ignore
/// #[restate_sdk::promise(Output = String)]
/// impl MyObject for MyStruct { ... }
/// ```
#[proc_macro_attribute]
pub fn promise(attr: TokenStream, input: TokenStream) -> TokenStream {
    // Parse optional Output = Type, defaulting to ()
    let output_ty: syn::Type = if attr.is_empty() {
        syn::parse_quote! { () }
    } else {
        let attr2 = proc_macro2::TokenStream::from(attr);
        match syn::parse2::<PromiseAttr>(attr2.clone()) {
            Ok(parsed) => parsed.output_ty,
            Err(_) => {
                return syn::Error::new_spanned(
                    attr2,
                    "expected `Output = Type` (e.g., `#[restate_sdk::promise(Output = String)]`)",
                )
                .to_compile_error()
                .into();
            }
        }
    };

    let item_impl = parse_macro_input!(input as syn::ItemImpl);

    // Extract the trait name from the impl block.
    let trait_path = match &item_impl.trait_ {
        Some((_, path, _)) => path,
        None => {
            return syn::Error::new_spanned(
                &item_impl,
                "promise requires a trait impl (e.g., `impl MyService for MyStruct`)",
            )
            .to_compile_error()
            .into();
        }
    };

    // Get the last segment of the trait path as the trait name.
    let trait_name = &trait_path
        .segments
        .last()
        .expect("trait path must have at least one segment")
        .ident;

    let client_ident = format_ident!("{}Client", trait_name);
    let self_ty = &item_impl.self_ty;

    let generated = quote! {
        impl ::restate_sdk::context::PromiseObject for #self_ty {
            type Output = #output_ty;
        }

        impl ::restate_sdk::context::LinkedObject for #self_ty {
            const SERVICE_NAME: &'static str = <#client_ident<'_> as ::restate_sdk::context::ServiceClient>::SERVICE_NAME;
        }
    };

    quote! {
        #item_impl
        #generated
    }
    .into()
}

/// Attribute macro for service trait implementations. Generates `NamedService`
/// impl connecting the concrete type to its handler name resolver.
///
/// ```ignore
/// #[restate_sdk::completion]
/// impl Agent for AgentImpl { ... }
/// ```
#[proc_macro_attribute]
pub fn completion(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let item_impl = parse_macro_input!(input as syn::ItemImpl);

    // Extract the trait name from the impl block.
    let trait_path = match &item_impl.trait_ {
        Some((_, path, _)) => path,
        None => {
            return syn::Error::new_spanned(
                &item_impl,
                "completion requires a trait impl (e.g., `impl MyService for MyStruct`)",
            )
            .to_compile_error()
            .into();
        }
    };

    // Get the last segment of the trait path as the trait name.
    let trait_name = &trait_path
        .segments
        .last()
        .expect("trait path must have at least one segment")
        .ident;

    let handler_resolution_ident = format_ident!("{}HandlerNameResolution", trait_name);
    let self_ty = &item_impl.self_ty;

    let named_completion = quote! {
        impl ::restate_sdk::context::handler::NamedService for #self_ty {
            type Resolution = #handler_resolution_ident;
        }
    };

    quote! {
        #item_impl
        #named_completion
    }
    .into()
}

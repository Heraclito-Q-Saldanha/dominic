use crate::*;

use std::collections;
use std::mem;

pub struct FunctionReferences {
	pub states: Vec<syn::Ident>,
	pub callbacks: Vec<syn::Ident>,
}

pub fn transpile_functions(body: &mut syn::Block, references: &collections::BTreeMap<String, FunctionReferences>) -> error::Result<proc_macro2::TokenStream> {
	let mut globals = quote::quote! {};
	let mut thread_locals = quote::quote! {};
	let mut initialization = quote::quote! {};

	for statement in mem::take(&mut body.stmts) {
		let syn::Stmt::Item(syn::Item::Fn(mut function)) = statement else {
			initialization = quote::quote! {
				#initialization
				#statement
			};
			continue;
		};

		let mut types = Vec::new();
		let mut arguments = Vec::new();

		for parameter in &function.sig.inputs {
			let syn::FnArg::Typed(parameter) = parameter else {
				return Err(syn::Error::new_spanned(&parameter, "Functions need to have typed parameters.").into());
			};
			let syn::Pat::Ident(argument) = parameter.pat.as_ref() else {
				return Err(syn::Error::new_spanned(&parameter.pat, "Function parameters need to be named bindings.").into());
			};

			types.push(&parameter.ty);
			arguments.push(&argument.ident);
		}

		let rs_name = function.sig.ident.to_string();

		let function_block = &function.block;
		let closure_body = match function.sig.safety {
			syn::Safety::Unsafe(_) => quote::quote!({ unsafe #function_block }),
			_ => quote::quote!(#function_block),
		};

		let uninitialized_error = format!("function `{rs_name}` called before initialization");
		let storage = quote::format_ident!("{}_closure_{}", &rs_name, fastrand::u128(..));

		let function_references = references.get(&rs_name);
		let states = function_references.map(|references| references.states.as_slice()).unwrap_or_default();
		let callbacks = function_references.map(|references| references.callbacks.as_slice()).unwrap_or_default();

		let is_async = function.sig.asyncness.is_some();
		let parameters = &function.sig.inputs;
		let output = &function.sig.output;

		match is_async {
			true => {
				function.block = syn::parse_quote!({
					let result = #storage.with_borrow_mut(|closure| {
						closure.as_mut().expect(#uninitialized_error)(#(#arguments),*)
					}).await;
					#(#callbacks().expect("Failed to update page reference");)*
					result
				});

				let return_type = match output {
					syn::ReturnType::Default => quote::quote!(()),
					syn::ReturnType::Type(_, return_type) => quote::quote!(#return_type),
				};

				thread_locals = quote::quote! {
					#[allow(non_upper_case_globals)]
					static #storage: ::std::cell::RefCell<::core::option::Option<::std::boxed::Box<dyn ::std::ops::FnMut(#(#types),*) -> ::core::pin::Pin<::std::boxed::Box<dyn ::core::future::Future<Output = #return_type>>>>>> = const { ::std::cell::RefCell::new(::core::option::Option::None) };

					#thread_locals
				};

				initialization = quote::quote! {
					#initialization
					{
						#(let #states = ::core::clone::Clone::clone(&#states);)*
						#storage.set(::core::option::Option::Some(::std::boxed::Box::new(move |#parameters| {
							#(let #states = ::core::clone::Clone::clone(&#states);)*
							::std::boxed::Box::pin(async move #closure_body)
						})));
					}
				};
			}
			false => {
				function.block = syn::parse_quote!({
					let result = #storage.with_borrow_mut(|closure| {
						closure.as_mut().expect(#uninitialized_error)(#(#arguments),*)
					});
					#(#callbacks().expect("Failed to update page reference");)*
					result
				});

				thread_locals = quote::quote! {
					#[allow(non_upper_case_globals)]
					static #storage: ::std::cell::RefCell<::core::option::Option<::std::boxed::Box<dyn ::std::ops::FnMut(#(#types),*) #output>>> = const { ::std::cell::RefCell::new(::core::option::Option::None) };

					#thread_locals
				};

				initialization = quote::quote! {
					#initialization
					{
						#(let #states = ::core::clone::Clone::clone(&#states);)*
						#storage.set(::core::option::Option::Some(::std::boxed::Box::new(move |#parameters| #output #closure_body)));
					}
				};
			}
		}

		globals = quote::quote! {
			#function
			#globals
		};
	}

	globals = quote::quote! {
		::std::thread_local! {
			#thread_locals
		}
		#globals
	};

	body.stmts = syn::parse_quote! {#initialization};

	Ok(globals)
}

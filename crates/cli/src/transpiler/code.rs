use crate::*;

use std::mem;

pub struct CodeTranspilerOutput {
	pub code: String,
	pub init_fn: String,
}

pub fn transpile_fn(html: &transpiler::html::HTMLTranspilerOutput, mut function: syn::ItemFn) -> error::Result<syn::Block> {
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

	if let Some(js_name) = html.references.get(&rs_name) {
		function.attrs.push(syn::parse_quote! {
			#[dominic_runtime::bindgen(js_name = #js_name, wasm_bindgen = dominic_runtime::wasm_bindgen)]
		});
		function.vis = syn::parse_quote!(pub);
	}

	let function_block = &function.block;
	let closure_body = match function.sig.safety {
		syn::Safety::Unsafe(_) => quote::quote!({ unsafe #function_block }),
		_ => quote::quote!(#function_block),
	};

	let uninitialized_error = format!("function `{rs_name}` called before initialization");
	let storage = quote::format_ident!("__{}_closure_{}", &rs_name, fastrand::u128(..));

	function.block = syn::parse_quote!({
		#storage.with_borrow_mut(|closure| {
			closure.as_mut().expect(#uninitialized_error)(#(#arguments),*)
		})
	});

	let parameters = &function.sig.inputs;
	let output = &function.sig.output;

	let replacement: syn::Block = syn::parse_quote!({
		::std::thread_local! {
			#[allow(non_upper_case_globals)]
			static #storage: ::std::cell::RefCell<::core::option::Option<::std::boxed::Box<dyn ::std::ops::FnMut(#(#types),*) #output>>> =
				const { ::std::cell::RefCell::new(::core::option::Option::None) };
		}

		#function

		#storage.set(::core::option::Option::Some(::std::boxed::Box::new(move |#parameters| #output #closure_body)));
	});

	Ok(replacement)
}

pub fn transpile_code(code: &str, html: &transpiler::html::HTMLTranspilerOutput) -> error::Result<CodeTranspilerOutput> {
	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	for statement in mem::take(&mut body.stmts) {
		match statement {
			syn::Stmt::Item(syn::Item::Fn(function)) => {
				let replacement = transpile_fn(html, function)?;

				body.stmts.extend(replacement.stmts);
			}
			statement => {
				body.stmts.push(statement);
			}
		}
	}

	let init_fn = quote::format_ident!("_init_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen)]
		pub fn #init_fn() #body
	}
	.to_string();

	let init_fn = init_fn.to_string();

	Ok(CodeTranspilerOutput { code, init_fn })
}

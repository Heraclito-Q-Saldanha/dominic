use crate::*;

use std::mem;

pub struct CodeTranspilerOutput {
	pub code: String,
	pub init_fn: String,
}

pub fn transpile_state_declarations(body: &mut syn::Block) -> error::Result<()> {
	for statement in mem::take(&mut body.stmts) {
		let syn::Stmt::Local(mut local) = statement else {
			body.stmts.push(statement);
			continue;
		};

		let Some(init) = &mut local.init else {
			body.stmts.push(syn::Stmt::Local(local));
			continue;
		};
		let syn::Expr::Macro(invocation) = init.expr.as_ref() else {
			body.stmts.push(syn::Stmt::Local(local));
			continue;
		};
		if !invocation.mac.path.is_ident("state") {
			body.stmts.push(syn::Stmt::Local(local));
			continue;
		}

		let value = invocation.mac.parse_body::<syn::Expr>()?;
		let attributes = &invocation.attrs;

		init.expr = syn::parse_quote!(#(#attributes)* ::dominic_runtime::State::new(#value));

		body.stmts.push(syn::Stmt::Local(local));
	}

	Ok(())
}

pub fn transpile_functions(html: &transpiler::html::HTMLTranspilerOutput, body: &mut syn::Block) -> error::Result<()> {
	for statement in mem::take(&mut body.stmts) {
		let syn::Stmt::Item(syn::Item::Fn(mut function)) = statement else {
			body.stmts.push(statement);
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

		body.stmts.extend(replacement.stmts);
	}

	Ok(())
}

pub fn transpile_code(code: &str, html: &transpiler::html::HTMLTranspilerOutput) -> error::Result<CodeTranspilerOutput> {
	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	transpile_functions(html, &mut body)?;
	transpile_state_declarations(&mut body)?;

	let init_fn = quote::format_ident!("_init_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen)]
		pub fn #init_fn() #body
	}
	.to_string();

	let init_fn = init_fn.to_string();

	Ok(CodeTranspilerOutput { code, init_fn })
}

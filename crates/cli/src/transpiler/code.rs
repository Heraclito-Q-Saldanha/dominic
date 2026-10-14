use crate::*;

use std::collections;
use std::mem;
use syn::visit_mut;

pub struct CodeTranspilerOutput {
	pub code: String,
	pub init_fn: String,
}

pub struct StateDeclaration {
	ident: syn::Ident,
	is_mutable: bool,
}

pub fn transpile_state_declarations(body: &mut syn::Block) -> error::Result<Vec<StateDeclaration>> {
	let mut states = Vec::new();

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
		let pattern = match &mut local.pat {
			syn::Pat::Type(pattern) => pattern.pat.as_mut(),
			pattern => pattern,
		};
		let syn::Pat::Ident(binding) = pattern else {
			return Err(syn::Error::new_spanned(&local.pat, "States need to be named bindings.").into());
		};
		states.push(StateDeclaration {
			ident: binding.ident.clone(),
			is_mutable: binding.mutability.take().is_some(),
		});

		init.expr = syn::parse_quote!(#(#attributes)* ::dominic_runtime::State::new(#value, []));

		body.stmts.push(syn::Stmt::Local(local));
	}

	Ok(states)
}

pub fn transpile_state_references(body: &mut syn::Block, states: &[StateDeclaration]) -> collections::BTreeMap<String, Vec<syn::Ident>> {
	struct StateReferences<'a> {
		states: &'a [StateDeclaration],
		references: Vec<syn::Ident>,
	}

	impl visit_mut::VisitMut for StateReferences<'_> {
		fn visit_expr_mut(&mut self, expression: &mut syn::Expr) {
			if let syn::Expr::Path(path) = expression {
				if let Some(state) = self.states.iter().find(|state| path.qself.is_none() && path.path.is_ident(&state.ident)) {
					if !self.references.contains(&state.ident) {
						self.references.push(state.ident.clone());
					}
					let attributes = &path.attrs;
					let name = &path.path;

					*expression = if state.is_mutable {
						syn::parse_quote!(#(#attributes)* (*#name.get_mut()))
					} else {
						syn::parse_quote!(#(#attributes)* (*#name.get()))
					};
					return;
				}
			}

			syn::visit_mut::visit_expr_mut(self, expression);
		}

		fn visit_field_value_mut(&mut self, field: &mut syn::FieldValue) {
			syn::visit_mut::visit_field_value_mut(self, field);
			if field.colon_token.is_none() && !matches!(field.expr, syn::Expr::Path(_)) {
				field.colon_token = Some(Default::default());
			}
		}

		fn visit_macro_mut(&mut self, invocation: &mut syn::Macro) {
			let Ok(mut arguments) = invocation.parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated) else {
				return;
			};
			for argument in &mut arguments {
				self.visit_expr_mut(argument);
			}
			invocation.tokens = quote::quote!(#arguments);
		}
	}

	let mut function_references = collections::BTreeMap::new();

	for statement in &mut body.stmts {
		let references = Vec::new();
		let mut references = StateReferences { states, references };

		visit_mut::VisitMut::visit_stmt_mut(&mut references, statement);
		if let syn::Stmt::Item(syn::Item::Fn(function)) = statement {
			function_references.insert(function.sig.ident.to_string(), references.references);
		}
	}

	function_references
}

pub fn transpile_functions(html: &transpiler::html::HTMLTranspilerOutput, body: &mut syn::Block, references: &collections::BTreeMap<String, Vec<syn::Ident>>) -> error::Result<()> {
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

		if let Some(js_name) = html.fn_references.get(&rs_name) {
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
		let storage = quote::format_ident!("_{}_closure_{}", &rs_name, fastrand::u128(..));

		function.block = syn::parse_quote!({
			#storage.with_borrow_mut(|closure| {
				closure.as_mut().expect(#uninitialized_error)(#(#arguments),*)
			})
		});

		let references = references.get(&rs_name).map(Vec::as_slice).unwrap_or_default();
		let parameters = &function.sig.inputs;
		let output = &function.sig.output;

		let replacement: syn::Block = syn::parse_quote!({
			::std::thread_local! {
				#[allow(non_upper_case_globals)]
				static #storage: ::std::cell::RefCell<::core::option::Option<::std::boxed::Box<dyn ::std::ops::FnMut(#(#types),*) #output>>> = const { ::std::cell::RefCell::new(::core::option::Option::None) };
			}

			#function

			#storage.set(::core::option::Option::Some({
				#(let #references = ::core::clone::Clone::clone(&#references);)*
				::std::boxed::Box::new(move |#parameters| #output #closure_body)
			}));
		});

		body.stmts.extend(replacement.stmts);
	}

	Ok(())
}

pub fn transpile_code(code: &str, html: &transpiler::html::HTMLTranspilerOutput) -> error::Result<CodeTranspilerOutput> {
	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	let states = transpile_state_declarations(&mut body)?;
	let references = transpile_state_references(&mut body, &states);
	transpile_functions(html, &mut body, &references)?;

	let init_fn = quote::format_ident!("_init_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen)]
		pub fn #init_fn() #body
	}
	.to_string();

	let init_fn = init_fn.to_string();

	Ok(CodeTranspilerOutput { code, init_fn })
}

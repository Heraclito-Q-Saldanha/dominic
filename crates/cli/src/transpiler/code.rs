use crate::*;

use std::collections;
use std::mem;
use syn::visit_mut;

pub struct CodeTranspilerOutput {
	pub code: String,
	pub init_fn: String,
}

pub fn transpile_segments(body: &mut syn::Block, html: &transpiler::html::HTMLTranspilerOutput) -> error::Result<Vec<syn::Ident>> {
	let mut functions = Vec::new();
	let mut exports = collections::BTreeMap::new();

	for (id, reference) in &html.references {
		let expression = syn::parse_str::<syn::Expr>(&reference.code)?;
		let name = quote::format_ident!("_update_segment_{id}");

		let update = match &reference.attribute {
			Some(attribute) => {
				let expression = match &expression {
					syn::Expr::Closure(closure) => {
						let name = quote::format_ident!("_attribute_{id}");
						let export = format!("_function_{:032x}", fastrand::u128(..));
						let inputs = &closure.inputs;
						let output = &closure.output;
						let block = &closure.body;

						body.stmts.push(syn::parse2(quote::quote! {
							#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen, js_name = #export)]
							pub fn #name(#inputs) #output { #block }
						})?);

						let call = format!("window._dominic_?.{export}()");

						exports.insert(name.to_string(), export);

						syn::parse_quote!(#call)
					}
					syn::Expr::Path(path) if path.qself.is_none() => {
						let function = body.stmts.iter_mut().find_map(|statement| match statement {
							syn::Stmt::Item(syn::Item::Fn(function)) if path.path.is_ident(&function.sig.ident) => Some(function),
							_ => None,
						});

						match function {
							Some(function) => {
								let export = exports.entry(function.sig.ident.to_string()).or_insert_with(|| {
									let name = format!("_function_{:032x}", fastrand::u128(..));

									function.vis = syn::parse_quote!(pub);
									function.attrs.push(syn::parse_quote! {
										#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen, js_name = #name)]
									});

									name
								});
								let call = format!("window._dominic_?.{export}()");

								syn::parse_quote!(#call)
							}
							None => expression,
						}
					}
					_ => expression,
				};

				quote::quote!(::dominic_runtime::dom::update_attribute(#id, #attribute, &::std::format!("{}", #expression)))
			}
			None => quote::quote!(::dominic_runtime::dom::update_segment(#id, &::std::format!("{}", #expression))),
		};

		body.stmts.push(syn::parse_quote! {
			fn #name() -> ::dominic_runtime::error::Result<()> {
				#update
			}
		});

		functions.push(name);
	}

	Ok(functions)
}

pub fn transpile_states(body: &mut syn::Block, updates: &[syn::Ident]) -> error::Result<collections::BTreeMap<String, Vec<syn::Ident>>> {
	struct StateDeclaration {
		ident: syn::Ident,
		is_mutable: bool,
		index: usize,
	}

	struct StateReferences<'a> {
		states: &'a [StateDeclaration],
		references: Vec<syn::Ident>,
		is_update: bool,
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

					*expression = if state.is_mutable && !self.is_update {
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

	let mut states = Vec::new();

	for (index, statement) in body.stmts.iter_mut().enumerate() {
		let syn::Stmt::Local(local) = statement else {
			continue;
		};

		let Some(init) = &mut local.init else {
			continue;
		};
		let syn::Expr::Macro(invocation) = init.expr.as_ref() else {
			continue;
		};
		if !invocation.mac.path.is_ident("state") {
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
			index,
		});

		init.expr = syn::parse_quote!(#(#attributes)* ::dominic_runtime::State::new(#value, []));
	}

	let mut function_references = collections::BTreeMap::new();

	for statement in &mut body.stmts {
		let mut visitor = StateReferences {
			states: &states,
			references: Vec::new(),
			is_update: matches!(statement, syn::Stmt::Item(syn::Item::Fn(function)) if updates.contains(&function.sig.ident)),
		};

		visit_mut::VisitMut::visit_stmt_mut(&mut visitor, statement);

		if let syn::Stmt::Item(syn::Item::Fn(function)) = statement {
			function_references.insert(function.sig.ident.to_string(), visitor.references);
		}
	}

	for state in &states {
		let callbacks = updates
			.iter()
			.filter(|name| function_references.get(&name.to_string()).is_some_and(|references| references.contains(&state.ident)));
		let syn::Stmt::Local(local) = &mut body.stmts[state.index] else {
			unreachable!();
		};
		let syn::Expr::Call(init) = local.init.as_mut().unwrap().expr.as_mut() else {
			unreachable!();
		};

		init.args[1] = syn::parse_quote!([#(|| #callbacks().expect("Failed to update page reference")),*]);
	}

	Ok(function_references)
}

pub fn transpile_functions(body: &mut syn::Block, references: &collections::BTreeMap<String, Vec<syn::Ident>>) -> error::Result<()> {
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

			{
				#(let #references = ::core::clone::Clone::clone(&#references);)*
				#storage.set(::core::option::Option::Some(::std::boxed::Box::new(move |#parameters| #output #closure_body)));
			}
		});

		body.stmts.extend(replacement.stmts);
	}

	Ok(())
}

pub fn transpile_code(code: &str, html: &transpiler::html::HTMLTranspilerOutput) -> error::Result<CodeTranspilerOutput> {
	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	let updates = transpile_segments(&mut body, html)?;
	let references = transpile_states(&mut body, &updates)?;
	transpile_functions(&mut body, &references)?;

	body.stmts.push(syn::parse_quote! {
		fn _init_segments() -> ::dominic_runtime::error::Result<()> {
			#(#updates()?;)*

			Ok(())
		}
	});

	body.stmts.push(syn::parse_quote! {
		_init_segments().expect("Failed to initialize page segments");
	});

	let init_fn = quote::format_ident!("_init_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen)]
		pub fn #init_fn() #body
	}
	.to_string();

	let init_fn = init_fn.to_string();

	Ok(CodeTranspilerOutput { code, init_fn })
}

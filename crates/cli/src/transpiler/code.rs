use crate::*;

use std::collections;
use std::mem;

use syn::parse::Parser;
use syn::visit_mut;

pub struct CodeTranspilerOutput {
	pub code: String,
	pub init_fn: String,
}

struct FunctionReferences {
	states: Vec<syn::Ident>,
	callbacks: Vec<syn::Ident>,
}

pub fn transpile_segments(body: &mut syn::Block, html: &transpiler::html::HTMLTranspilerOutput<'_>) -> error::Result<Vec<syn::Ident>> {
	let mut functions = Vec::new();
	let mut exports = collections::BTreeMap::new();

	for (id, reference) in &html.references {
		let name = quote::format_ident!("_update_segment_{id}");

		let update = match reference {
			dominic_parser::Node::AttributeReference { name: attribute, code } => {
				let expression = syn::parse_str::<syn::Expr>(code)?;

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
			dominic_parser::Node::CodeReference(code) => {
				let expression = syn::parse_str::<syn::Expr>(code)?;

				quote::quote!(::dominic_runtime::dom::update_segment(#id, &::std::format!("{}", #expression), false))
			}
			dominic_parser::Node::If { condition, if_body: nodes, else_body } => {
				let condition = syn::parse_str::<syn::Expr>(condition)?;
				let if_body = render_nodes(nodes)?;
				let else_body = match else_body {
					Some(nodes) => render_nodes(nodes)?,
					None => syn::parse_quote!({ ::std::string::String::new() }),
				};

				quote::quote!({
					let content = if #condition #if_body else #else_body;
					::dominic_runtime::dom::update_segment(#id, &content, true)
				})
			}
			dominic_parser::Node::For { pattern, items, body: nodes } => {
				let pattern = syn::Pat::parse_single.parse_str(pattern)?;
				let items = syn::parse_str::<syn::Expr>(items)?;
				let body = render_nodes(nodes)?;

				quote::quote!({
					let mut content = ::std::string::String::new();
					for #pattern in #items {
						content.push_str(&(#body));
					}
					::dominic_runtime::dom::update_segment(#id, &content, true)
				})
			}
			_ => unreachable!("Only dynamic nodes have references"),
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

fn render_nodes(nodes: &[dominic_parser::Node<'_>]) -> error::Result<syn::Block> {
	let mut statements: Vec<syn::Stmt> = Vec::new();
	let mut static_html = String::new();

	for node in nodes {
		match node {
			dominic_parser::Node::Static(value) => static_html.push_str(value),
			dominic_parser::Node::CodeReference(code) => {
				let value = mem::take(&mut static_html);
				let expression = syn::parse_str::<syn::Expr>(code)?;

				statements.push(syn::parse_quote!(html.push_str(#value);));
				statements.push(syn::parse_quote! {
					html.push_str(&::std::format!("{}", #expression));
				});
			}
			dominic_parser::Node::AttributeReference { name, code } => {
				let start = static_html.trim_end().strip_suffix('=').unwrap().trim_end().len() - name.len();

				static_html.truncate(start);

				let value = mem::take(&mut static_html);
				let expression = syn::parse_str::<syn::Expr>(code)?;
				let name = *name;

				statements.push(syn::parse_quote!(html.push_str(#value);));
				statements.push(syn::parse_quote! {
					html.push_str(#name);
					html.push_str("=\"");
					html.push_str(&::std::format!("{}", #expression));
					html.push('"');
				});
			}
			dominic_parser::Node::If { condition, if_body, else_body } => {
				let value = mem::take(&mut static_html);
				let condition = syn::parse_str::<syn::Expr>(condition)?;

				statements.push(syn::parse_quote!(html.push_str(#value);));

				let if_body = render_nodes(if_body)?;
				let else_body = match else_body {
					Some(nodes) => Some(render_nodes(nodes)?),
					None => None,
				};

				if let Some(else_body) = else_body {
					statements.push(syn::parse_quote! {
						if #condition {
							html.push_str(&(#if_body));
						} else {
							html.push_str(&(#else_body));
						}
					});
				} else {
					statements.push(syn::parse_quote! {
						if #condition {
							html.push_str(&(#if_body));
						}
					});
				}
			}
			dominic_parser::Node::For { pattern, items, body } => {
				let value = mem::take(&mut static_html);
				let pattern = syn::Pat::parse_single.parse_str(pattern)?;
				let items = syn::parse_str::<syn::Expr>(items)?;
				let body = render_nodes(body)?;

				statements.push(syn::parse_quote!(html.push_str(#value);));
				statements.push(syn::parse_quote! {
					for #pattern in #items {
						html.push_str(&(#body));
					}
				});
			}
			_ => {}
		}
	}

	statements.push(syn::parse_quote!(html.push_str(#static_html);));

	Ok(syn::parse_quote!({
		let mut html = ::std::string::String::new();
		#(#statements)*
		html
	}))
}

fn transpile_states(body: &mut syn::Block, updates: &[syn::Ident]) -> error::Result<collections::BTreeMap<String, FunctionReferences>> {
	struct StateDeclaration {
		ident: syn::Ident,
		is_mutable: bool,
	}

	struct StateReferences<'a> {
		states: &'a [StateDeclaration],
		references: Vec<syn::Ident>,
		read_only: bool,
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

					*expression = if state.is_mutable && !self.read_only {
						syn::parse_quote!(#(#attributes)* *#name.get_mut())
					} else {
						syn::parse_quote!(#(#attributes)* *#name.get())
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

	for statement in &mut body.stmts {
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
		});

		init.expr = syn::parse_quote!(#(#attributes)* ::dominic_runtime::state::State::new(#value));
	}

	let mut function_references = collections::BTreeMap::new();

	for statement in &mut body.stmts {
		let mut visitor = StateReferences {
			states: &states,
			references: Vec::new(),
			read_only: matches!(statement, syn::Stmt::Item(syn::Item::Fn(function)) if updates.contains(&function.sig.ident)),
		};

		visit_mut::VisitMut::visit_stmt_mut(&mut visitor, statement);

		if let syn::Stmt::Item(syn::Item::Fn(function)) = statement {
			function_references.insert(
				function.sig.ident.to_string(),
				FunctionReferences {
					states: visitor.references,
					callbacks: Vec::new(),
				},
			);
		}
	}

	let mut update_states = Vec::new();

	for update in updates {
		let update_name = update.to_string();
		let Some(references) = function_references.get(&update_name) else {
			continue;
		};

		let mut mutable_states = Vec::new();
		for state in &states {
			if state.is_mutable && references.states.contains(&state.ident) {
				mutable_states.push(state.ident.clone());
			}
		}

		update_states.push((update.clone(), mutable_states));
	}

	for (name, references) in &mut function_references {
		let is_update_function = updates.iter().any(|update| update.to_string() == *name);

		if is_update_function {
			continue;
		}

		for (update, mutable_states) in &update_states {
			let uses_mutable_state = references.states.iter().any(|state| mutable_states.contains(state));
			if uses_mutable_state {
				references.callbacks.push(update.clone());
			}
		}
	}

	Ok(function_references)
}

fn transpile_functions(body: &mut syn::Block, references: &collections::BTreeMap<String, FunctionReferences>) -> error::Result<proc_macro2::TokenStream> {
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

		function.block = syn::parse_quote!({
			let result = #storage.with_borrow_mut(|closure| {
				closure.as_mut().expect(#uninitialized_error)(#(#arguments),*)
			});
			#(#callbacks().expect("Failed to update page reference");)*
			result
		});

		let parameters = &function.sig.inputs;
		let output = &function.sig.output;

		thread_locals = syn::parse_quote! {
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

pub fn transpile_code(code: &str, html: &transpiler::html::HTMLTranspilerOutput) -> error::Result<CodeTranspilerOutput> {
	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	let updates = transpile_segments(&mut body, html)?;
	let references = transpile_states(&mut body, &updates)?;
	let mut globals = transpile_functions(&mut body, &references)?;

	globals = quote::quote! {
		#globals
		fn _init_segments() -> ::dominic_runtime::error::Result<()> {
			#(#updates()?;)*

			Ok(())
		}
	};

	body.stmts.push(syn::parse_quote! {
		_init_segments().expect("Failed to initialize page segments");
	});

	let init_fn = quote::format_ident!("_init_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#globals
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen)]
		pub fn #init_fn() #body
	};

	let ast = syn::parse2(code)?;
	let code = prettyplease::unparse(&ast);
	let init_fn = init_fn.to_string();

	Ok(CodeTranspilerOutput { code, init_fn })
}

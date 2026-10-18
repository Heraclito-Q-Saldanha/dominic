use crate::transpiler;

use std::collections;
use std::mem;

use syn::parse::Parser;

pub fn transpile_segments(body: &mut syn::Block, html: &transpiler::HTMLTranspilerOutput<'_>) -> crate::error::Result<Vec<syn::Ident>> {
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

fn render_nodes(nodes: &[dominic_parser::Node<'_>]) -> crate::error::Result<syn::Block> {
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

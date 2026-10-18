mod function;
mod segment;
mod state;

use crate::*;

use std::collections;

pub struct HTMLTranspilerOutput<'source> {
	pub html: String,
	pub references: collections::BTreeMap<usize, dominic_parser::Node<'source>>,
}

pub struct CodeTranspilerOutput {
	pub code: String,
	pub init_fn: String,
}

pub fn transpile(input: &str) -> error::Result<(HTMLTranspilerOutput<'_>, CodeTranspilerOutput)> {
	let input = dominic_parser::parse(input).map_err(|error| error::Error::Parser(error.to_string()))?;
	let mut code = String::new();

	for node in &input.nodes {
		if let dominic_parser::Node::Script(script) = node {
			code.push_str(script);
			code.push('\n');
		}
	}

	let html_output = transpile_html(&input);
	let code_output = transpile_code(&code, &html_output)?;

	Ok((html_output, code_output))
}

fn transpile_code(code: &str, html: &HTMLTranspilerOutput) -> error::Result<CodeTranspilerOutput> {
	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	let updates = segment::transpile_segments(&mut body, html)?;
	let references = state::transpile_states(&mut body, &updates)?;
	let mut globals = function::transpile_functions(&mut body, &references)?;

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

	let init_fn = format!("_init_page_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#globals
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen, wasm_bindgen_futures = dominic_runtime::wasm_bindgen_futures, js_name=#init_fn)]
		pub async fn _init_page() #body
	};

	let ast = syn::parse2(code)?;
	let code = prettyplease::unparse(&ast);

	Ok(CodeTranspilerOutput { code, init_fn })
}

pub fn transpile_html<'source>(input: &dominic_parser::Code<'source>) -> HTMLTranspilerOutput<'source> {
	let mut references = collections::BTreeMap::new();
	let mut html = String::with_capacity(input.source.len());

	for node in &input.nodes {
		match node {
			dominic_parser::Node::Static(content) => html.push_str(content),
			dominic_parser::Node::AttributeReference { name, .. } => {
				let id = references.len();
				let start = html.trim_end().strip_suffix('=').unwrap().trim_end().len() - name.len();

				html.truncate(start);
				html.push_str(&format!("data-dominic-{id}=\"\""));

				references.insert(id, node.clone());
			}
			dominic_parser::Node::CodeReference(_) => {
				let id = references.len();

				html.push_str(&format!("<!--dominic:{id}--><!--/dominic:{id}-->"));

				references.insert(id, node.clone());
			}
			dominic_parser::Node::If { .. } => {
				let id = references.len();

				html.push_str(&format!("<!--dominic:{id}--><!--/dominic:{id}-->"));
				references.insert(id, node.clone());
			}
			dominic_parser::Node::For { .. } => {
				let id = references.len();

				html.push_str(&format!("<!--dominic:{id}--><!--/dominic:{id}-->"));
				references.insert(id, node.clone());
			}
			_ => {}
		}
	}

	html.truncate(html.trim_end().len());
	html.drain(..html.len() - html.trim_start().len());

	HTMLTranspilerOutput { html, references }
}

use std::collections;

pub struct HTMLTranspilerOutput {
	pub html: String,
	pub fn_references: collections::BTreeMap<String, String>,
}

pub fn transpile_html(input: &dominic_parser::Code<'_>) -> HTMLTranspilerOutput {
	let mut fn_references = collections::BTreeMap::new();
	let mut html = String::with_capacity(input.source.len());

	for token in &input.tokens {
		match token {
			dominic_parser::Token::Static(content) => html.push_str(content),
			dominic_parser::Token::CodeReference(rs_name) | dominic_parser::Token::AttributeReference { code: rs_name, .. } => {
				let js_name = fn_references.entry((*rs_name).to_string()).or_insert_with(|| format!("_{}_{}", rs_name, fastrand::u128(..)));
				html.push_str(&format!("window._dominic_?.{js_name}()"));
			}
			_ => {}
		}
	}

	html.truncate(html.trim_end().len());
	html.drain(..html.len() - html.trim_start().len());

	HTMLTranspilerOutput { html, fn_references }
}

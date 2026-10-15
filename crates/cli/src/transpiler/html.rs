use std::collections;

pub struct Reference {
	pub code: String,
	pub attribute: Option<String>,
}

pub struct HTMLTranspilerOutput {
	pub html: String,
	pub references: collections::BTreeMap<usize, Reference>,
}

pub fn transpile_html(input: &dominic_parser::Code<'_>) -> HTMLTranspilerOutput {
	let mut references = collections::BTreeMap::new();
	let mut html = String::with_capacity(input.source.len());

	for token in &input.tokens {
		match token {
			dominic_parser::Token::Static(content) => html.push_str(content),
			dominic_parser::Token::AttributeReference { name, code } => {
				let id = references.len();
				let start = html.trim_end().strip_suffix('=').unwrap().trim_end().len() - name.len();

				html.truncate(start);
				html.push_str(&format!("data-dominic-{id}=\"\""));

				let code = code.to_string();
				let attribute = Some(name.to_string());

				references.insert(id, Reference { code, attribute });
			}
			dominic_parser::Token::CodeReference(code) => {
				let id = references.len();

				html.push_str(&format!("<!--dominic:{id}--><!--/dominic:{id}-->"));

				let code = code.to_string();
				let attribute = None;

				references.insert(id, Reference { code, attribute });
			}
			_ => {}
		}
	}

	html.truncate(html.trim_end().len());
	html.drain(..html.len() - html.trim_start().len());

	HTMLTranspilerOutput { html, references }
}

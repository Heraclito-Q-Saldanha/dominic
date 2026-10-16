use std::collections;

pub struct HTMLTranspilerOutput<'source> {
	pub html: String,
	pub references: collections::BTreeMap<usize, dominic_parser::Node<'source>>,
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

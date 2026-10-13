use crate::*;

use std::collections;

pub struct HTMLTranspilerOutput {
	pub html: String,
	pub fn_references: collections::BTreeMap<String, String>,
}

pub fn transpile_html(input: &str) -> error::Result<HTMLTranspilerOutput> {
	let mut fn_references = collections::BTreeMap::new();

	let html = lol_html::rewrite_str(
		input,
		lol_html::RewriteStrSettings::new().append_element_content_handler(lol_html::element!("*", |element| {
			let mut attributes = Vec::new();

			for attribute in element.attributes() {
				let value = attribute.value();
				let Some(value) = value.trim().strip_prefix('{') else {
					continue;
				};
				let Some(value) = value.strip_suffix('}') else {
					continue;
				};
				let rs_name = value.trim();
				let js_name = format!("_{}_{}", rs_name, fastrand::u128(..));

				fn_references.insert(rs_name.to_string(), js_name.clone());
				attributes.push((attribute.name(), format!("window._dominic_?.{js_name}()")));
			}

			for (name, value) in attributes {
				element.set_attribute(&name, &value)?;
			}

			Ok(())
		})),
	)?;

	Ok(HTMLTranspilerOutput { html, fn_references })
}

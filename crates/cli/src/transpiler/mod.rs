use crate::*;

use std::collections;

fn split_code(input: &str) -> error::Result<(String, String)> {
	let mut code = String::new();

	let mut html = lol_html::rewrite_str(
		input,
		lol_html::RewriteStrSettings::new()
			.append_element_content_handler(lol_html::element!("script", |element| {
				element.remove();

				Ok(())
			}))
			.append_element_content_handler(lol_html::text!("script", |text| {
				code.push_str(text.as_str());

				Ok(())
			})),
	)?;

	html.truncate(html.trim_end().len());
	html.drain(..html.len() - html.trim_start().len());

	Ok((code, html))
}

pub fn transpile(input: &str) -> error::Result<(String, String)> {
	let (code, html) = split_code(input)?;

	let mut references = collections::BTreeMap::new();

	let mut html = lol_html::rewrite_str(
		&html,
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

				references.insert(rs_name.to_string(), js_name.clone());
				attributes.push((attribute.name(), format!("window.__dominic?.{js_name}()")));
			}

			for (name, value) in attributes {
				element.set_attribute(&name, &value)?;
			}

			Ok(())
		})),
	)?;

	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	for statement in &mut body.stmts {
		let syn::Stmt::Item(syn::Item::Fn(function)) = statement else {
			continue;
		};
		let rs_name = function.sig.ident.to_string();

		let Some(js_name) = references.get(&rs_name) else {
			continue;
		};

		function.attrs.push(syn::parse_quote! {
			#[dominic_runtime::bindgen(js_name = #js_name, wasm_bindgen = dominic_runtime::wasm_bindgen)]
		});
		function.vis = syn::parse_quote!(pub);
	}

	let init_fn = quote::format_ident!("__init_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen)]
		pub fn #init_fn() #body
	}
	.to_string();

	let script = format!(
		r#"
<script type="module">
	import * as script from "/index.js";

	const start = async () => {{
		await script.default({{ module_or_path: "/index_bg.wasm" }});
		window.__dominic = script;
		window.__dominic.{init_fn}();
	}};
	if (document.readyState === "complete") {{
		start();
	}} else {{
		window.addEventListener("load", start, {{ once: true }});
	}}
</script>
"#
	);

	html.push_str(&script);

	Ok((code, html))
}

use crate::*;

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
	let init_fn = quote::format_ident!("__init_{:032x}", fastrand::u128(..));

	let code = transpile_code(&code, &init_fn)?;
	let html = transpile_html(html, &init_fn)?;

	Ok((code, html))
}

fn transpile_code(code: &str, init_fn: &syn::Ident) -> error::Result<String> {
	let body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;
	let code = quote::quote! {
		#[unsafe(no_mangle)]
		pub extern "C" fn #init_fn() #body
	}
	.to_string();

	Ok(code)
}

fn transpile_html(mut html: String, init_fn: &syn::Ident) -> error::Result<String> {
	let script = format!(
		r#"
<script>
(() => {{
	const init = async () => {{
		const {{ instance }} = await WebAssembly.instantiateStreaming(fetch("/app.wasm"));
		instance.exports.{init_fn}();
	}};
	const start = () => init().catch(console.error);
	if (document.readyState === "complete") {{
		start();
	}} else {{
		window.addEventListener("load", start, {{ once: true }});
	}}
}})();
</script>
"#
	);

	html.push_str(&script);

	Ok(html)
}

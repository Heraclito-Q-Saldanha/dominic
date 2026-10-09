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
				code.push_str(text.as_str().trim());

				Ok(())
			})),
	)?;

	html.truncate(html.trim_end().len());
	html.drain(..html.len() - html.trim_start().len());

	Ok((code, html))
}

pub fn transpile(input: &str) -> error::Result<(String, String)> {
	let (code, html) = split_code(input)?;

	let code = transpile_code(&code)?;
	let html = transpile_html(&html)?;

	Ok((code, html))
}

fn transpile_code(code: &str) -> error::Result<String> {
	Ok(code.to_string())
}

fn transpile_html(html: &str) -> error::Result<String> {
	Ok(html.to_string())
}

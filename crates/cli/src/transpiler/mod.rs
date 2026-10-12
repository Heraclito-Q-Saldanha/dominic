mod code;
mod html;

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

pub fn transpile(input: &str) -> error::Result<(html::HTMLTranspilerOutput, code::CodeTranspilerOutput)> {
	let (code, html) = split_code(input)?;

	let html_output = html::transpile_html(&html)?;
	let code_output = code::transpile_code(&code, &html_output)?;

	Ok((html_output, code_output))
}

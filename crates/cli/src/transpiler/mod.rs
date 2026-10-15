mod code;
mod html;

use crate::*;

pub fn transpile(input: &str) -> error::Result<(html::HTMLTranspilerOutput, code::CodeTranspilerOutput)> {
	let input = dominic_parser::parse(input).map_err(|error| error::Error::Parser(error.to_string()))?;
	let mut code = String::new();

	for token in &input.tokens {
		if let dominic_parser::Token::Script(script) = token {
			code.push_str(script);
			code.push('\n');
		}
	}

	let html_output = html::transpile_html(&input);
	let code_output = code::transpile_code(&code)?;

	Ok((html_output, code_output))
}

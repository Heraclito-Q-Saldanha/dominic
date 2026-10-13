mod code;
mod html;

use crate::*;

pub fn transpile(input: &str) -> error::Result<(html::HTMLTranspilerOutput, code::CodeTranspilerOutput)> {
	let input = dominic_lexer::parse(input).map_err(|()| error::Error::Lexer)?;
	let mut code = String::new();
	for token in &input.tokens {
		if let dominic_lexer::Token::Script(script) = token {
			code.push_str(script);
			code.push('\n');
		}
	}

	let html_output = html::transpile_html(&input);
	let code_output = code::transpile_code(&code, &html_output)?;

	Ok((html_output, code_output))
}

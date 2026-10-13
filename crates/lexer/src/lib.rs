use logos::Logos;

pub struct Code<'source> {
	pub source: &'source str,
	pub tokens: Vec<Token<'source>>,
}

pub fn parse<'source>(source: &'source str) -> Result<Code<'source>, ()> {
	let tokens = Token::lexer(source).collect::<Result<Vec<_>, _>>()?;

	Ok(Code { source, tokens })
}

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token<'source> {
	#[token("<script>", |lex| block_content(lex, "</script>"))]
	Script(&'source str),
	#[token("<style>", |lex| block_content(lex, "</style>"))]
	Style(&'source str),
	#[token("{", |lex| block_content(lex, "}"))]
	CodeReference(&'source str),
	#[regex(r"[^<{]+|<", static_content)]
	Static(&'source str),
}

fn static_content<'source>(lexer: &mut logos::Lexer<'source, Token<'source>>) -> &'source str {
	let mut length = lexer.remainder().len();
	for delimiter in ["<script>", "<style>", "{"] {
		if let Some(index) = lexer.remainder().find(delimiter) {
			length = length.min(index);
		}
	}
	lexer.bump(length);
	lexer.slice()
}

fn block_content<'source>(lexer: &mut logos::Lexer<'source, Token<'source>>, closing: &str) -> Result<&'source str, ()> {
	let source = lexer.remainder();
	let Some(end) = source.find(closing) else {
		lexer.bump(source.len());
		return Err(());
	};
	lexer.bump(end + closing.len());
	Ok(source[..end].trim())
}

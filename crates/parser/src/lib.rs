use nom::Parser;
use nom::bytes::complete::*;
use nom::character::complete::*;
use nom::combinator::*;

pub type Error<'a> = nom::Err<nom::error::Error<&'a str>>;
pub type Result<'a, T, E = Error<'a>> = ::core::result::Result<T, E>;

type SpannedToken<'source> = (&'source str, Token<'source>);

#[derive(Debug)]
pub struct Code<'source> {
	pub source: &'source str,
	pub tokens: Vec<Token<'source>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token<'source> {
	Script(&'source str),
	Style(&'source str),
	CodeReference(&'source str),
	AttributeReference { name: &'source str, code: &'source str },
	Static(&'source str),
}

pub fn parse(source: &str) -> Result<'_, Code<'_>> {
	let mut input = source;
	let mut tokens = Vec::new();
	let mut cursor = 0;

	while !input.is_empty() {
		let (rest, parsed) = nom::branch::alt((
			consumed(block("<script>", "</script>")).map(|(span, code)| vec![(span, Token::Script(code))]),
			consumed(block("<style>", "</style>")).map(|(span, code)| vec![(span, Token::Style(code))]),
			block("<!--", "-->").map(|_| Vec::new()),
			element,
			reference.map(|(span, code)| vec![(span, Token::CodeReference(code))]),
			nom::branch::alt((is_not("<{"), tag("<"))).map(|_| Vec::new()),
		))
		.parse(input)?;

		for (span, token) in parsed {
			let start = nom::Offset::offset(source, span);
			if cursor < start {
				tokens.push(Token::Static(&source[cursor..start]));
			}
			tokens.push(token);
			cursor = start + span.len();
		}
		input = rest;
	}

	if cursor < source.len() {
		tokens.push(Token::Static(&source[cursor..]));
	}

	Ok(Code { source, tokens })
}

fn block<'source>(opening: &'static str, closing: &'static str) -> impl Parser<&'source str, Output = &'source str, Error = nom::error::Error<&'source str>> {
	nom::sequence::preceded(tag(opening), cut(nom::sequence::terminated(nom::bytes::complete::take_until(closing), tag(closing)))).map(str::trim)
}

fn reference(input: &str) -> Result<'_, (&str, (&str, &str))> {
	consumed(block("{", "}")).parse(input)
}

fn element(input: &str) -> Result<'_, (&str, Vec<SpannedToken<'_>>)> {
	let (input, _) = (
		char('<'),
		opt(char('/')),
		nom::character::complete::satisfy(|c| c.is_ascii_alphabetic()),
		nom::bytes::complete::take_while(|c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | ':')),
	)
		.parse(input)?;

	let (input, (attributes, _, _, _)) = cut((
		nom::multi::many0(nom::sequence::preceded(nom::character::complete::multispace1, attribute)),
		multispace0,
		opt(char('/')),
		char('>'),
	))
	.parse(input)?;

	Ok((input, attributes.into_iter().flatten().collect()))
}

fn attribute(input: &str) -> Result<'_, (&str, Option<SpannedToken<'_>>)> {
	let (input, name) = nom::bytes::complete::take_while1(|c: char| !c.is_ascii_whitespace() && !matches!(c, '=' | '>' | '/' | '"' | '\'' | '<' | '{')).parse(input)?;
	let (input, value) = opt(nom::sequence::preceded((multispace0, char('='), multispace0), cut(attribute_value))).parse(input)?;
	let token = value.flatten().map(|(span, code)| (span, Token::AttributeReference { name, code }));

	Ok((input, token))
}

fn attribute_value(input: &str) -> Result<'_, (&str, Option<(&str, &str)>)> {
	nom::branch::alt((
		block("\"", "\"").map(|_| None),
		block("'", "'").map(|_| None),
		reference.map(Some),
		is_not(" \t\r\n>\"'<={}").map(|_| None),
	))
	.parse(input)
}

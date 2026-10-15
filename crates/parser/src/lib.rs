use nom::Parser;
use nom::bytes::complete::*;
use nom::character::complete::*;
use nom::combinator::*;

pub type Error<'a> = nom::Err<nom::error::Error<&'a str>>;
pub type Result<'a, T, E = Error<'a>> = ::core::result::Result<T, E>;

type SpannedNode<'source> = (&'source str, Node<'source>);

#[derive(Debug)]
pub struct Code<'source> {
	pub source: &'source str,
	pub nodes: Vec<Node<'source>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node<'source> {
	Script(&'source str),
	Style(&'source str),
	CodeReference(&'source str),
	AttributeReference {
		name: &'source str,
		code: &'source str,
	},
	If {
		condition: &'source str,
		body: Vec<Node<'source>>,
		else_body: Option<Vec<Node<'source>>>,
	},
	Each {
		items: &'source str,
		item: &'source str,
		index: Option<&'source str>,
		body: Vec<Node<'source>>,
	},
	Static(&'source str),
}

pub fn parse(source: &str) -> Result<'_, Code<'_>> {
	let (_, nodes) = all_consuming(nodes).parse(source)?;

	Ok(Code { source, nodes })
}

fn nodes(source: &str) -> Result<'_, (&str, Vec<Node<'_>>)> {
	let mut input = source;
	let mut nodes = Vec::new();
	let mut cursor = 0;

	while !input.is_empty() && !input.starts_with("{:") && !input.starts_with("{/") {
		let (rest, parsed) = nom::branch::alt((
			consumed(block("<script>", "</script>")).map(|(span, code)| vec![(span, Node::Script(code))]),
			consumed(block("<style>", "</style>")).map(|(span, code)| vec![(span, Node::Style(code))]),
			block("<!--", "-->").map(|_| Vec::new()),
			element,
			directive.map(|node| vec![node]),
			reference.map(|(span, code)| vec![(span, Node::CodeReference(code))]),
			nom::branch::alt((is_not("<{"), tag("<"))).map(|_| Vec::new()),
		))
		.parse(input)?;

		for (span, node) in parsed {
			let start = nom::Offset::offset(source, span);

			if cursor < start {
				nodes.push(Node::Static(&source[cursor..start]));
			}

			nodes.push(node);
			cursor = start + span.len();
		}
		input = rest;
	}

	let end = source.len() - input.len();
	if cursor < end {
		nodes.push(Node::Static(&source[cursor..end]));
	}

	Ok((input, nodes))
}

fn block<'source>(opening: &'static str, closing: &'static str) -> impl Parser<&'source str, Output = &'source str, Error = nom::error::Error<&'source str>> {
	nom::sequence::preceded(tag(opening), cut(nom::sequence::terminated(nom::bytes::complete::take_until(closing), tag(closing)))).map(str::trim)
}

fn reference(input: &str) -> Result<'_, (&str, (&str, &str))> {
	consumed(block("{", "}")).parse(input)
}

fn directive(input: &str) -> Result<'_, (&str, SpannedNode<'_>)> {
	peek(tag("{#")).parse(input)?;
	consumed(cut(nom::branch::alt((conditional, each)))).parse(input)
}

fn conditional(input: &str) -> Result<'_, (&str, Node<'_>)> {
	let (input, (_, _, condition, _)) = (tag("{#if"), multispace1, verify(take_until("}"), |condition: &str| !condition.trim().is_empty()), char('}')).parse(input)?;
	let (input, body) = nodes(input)?;
	let (input, (_, closing)) = reference(input)?;

	let (input, else_body) = match closing {
		"/if" => (input, None),
		":else" => {
			let (input, body) = nodes(input)?;
			let (input, _) = verify(reference, |(_, closing)| *closing == "/if").parse(input)?;

			(input, Some(body))
		}
		_ => return Err(nom::Err::Error(nom::error::Error::new(closing, nom::error::ErrorKind::Tag))),
	};

	Ok((
		input,
		Node::If {
			condition: condition.trim(),
			body,
			else_body,
		},
	))
}

fn each(input: &str) -> Result<'_, (&str, Node<'_>)> {
	let (input, (_, _, header, _)) = (tag("{#each"), multispace1, take_until("}"), char('}')).parse(input)?;
	let Some((separator, _)) = header
		.rmatch_indices("as")
		.find(|(index, _)| header[..*index].ends_with(char::is_whitespace) && header[index + 2..].starts_with(char::is_whitespace))
	else {
		return Err(nom::Err::Error(nom::error::Error::new(header, nom::error::ErrorKind::Tag)));
	};

	let items = header[..separator].trim();

	if items.is_empty() {
		return Err(nom::Err::Error(nom::error::Error::new(header, nom::error::ErrorKind::Verify)));
	}

	let (_, (item, index, _)) =
		all_consuming((identifier, opt(nom::sequence::preceded((multispace0, char(','), multispace0), identifier)), multispace0)).parse(header[separator + 2..].trim_start())?;
	let (input, body) = nodes(input)?;
	let (input, _) = verify(reference, |(_, closing)| *closing == "/each").parse(input)?;

	Ok((input, Node::Each { items, item, index, body }))
}

fn identifier(input: &str) -> Result<'_, (&str, &str)> {
	recognize((satisfy(|c| c == '_' || c.is_alphabetic()), take_while(|c: char| c == '_' || c.is_alphanumeric()))).parse(input)
}

fn element(input: &str) -> Result<'_, (&str, Vec<SpannedNode<'_>>)> {
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

fn attribute(input: &str) -> Result<'_, (&str, Option<SpannedNode<'_>>)> {
	let (input, name) = nom::bytes::complete::take_while1(|c: char| !c.is_ascii_whitespace() && !matches!(c, '=' | '>' | '/' | '"' | '\'' | '<' | '{')).parse(input)?;
	let (input, value) = opt(nom::sequence::preceded((multispace0, char('='), multispace0), cut(attribute_value))).parse(input)?;
	let node = value.flatten().map(|(span, code)| (span, Node::AttributeReference { name, code }));

	Ok((input, node))
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

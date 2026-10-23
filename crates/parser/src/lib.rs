use nom::Parser;
use nom::bytes::complete::*;
use nom::character::complete::*;
use nom::combinator::*;

pub type Error<'a> = nom::Err<nom::error::Error<&'a str>>;
pub type Result<'a, T, E = Error<'a>> = core::result::Result<T, E>;

type SpannedNode<'source> = (&'source str, Node<'source>);

#[derive(Debug)]
pub struct Code<'source> {
	pub source: &'source str,
	pub nodes: Vec<Node<'source>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentProp<'source> {
	pub name: &'source str,
	pub value: &'source str,
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
		if_body: Vec<Node<'source>>,
		else_body: Option<Vec<Node<'source>>>,
	},
	For {
		pattern: &'source str,
		items: &'source str,
		body: Vec<Node<'source>>,
	},
	Component {
		name: &'source str,
		props: Vec<ComponentProp<'source>>,
		children: Vec<Node<'source>>,
	},
	Static(&'source str),
}

pub fn parse(source: &str) -> Result<'_, Code<'_>> {
	let (_, nodes) = all_consuming(|source| nodes(source, None)).parse(source)?;

	Ok(Code { source, nodes })
}

fn nodes_end(input: &str, component_name: Option<&str>) -> bool {
	input.starts_with("{:") || input.starts_with("{/") || component_name.is_some_and(|name| component_end(input, name).is_ok())
}

fn nodes<'source>(source: &'source str, component_name: Option<&str>) -> Result<'source, (&'source str, Vec<Node<'source>>)> {
	let mut input = source;
	let mut nodes = Vec::new();
	let mut cursor = 0;

	while !input.is_empty() && !nodes_end(input, component_name) {
		let (rest, parsed) = nom::branch::alt((
			consumed(block("<script>", "</script>")).map(|(span, code)| vec![(span, Node::Script(code))]),
			consumed(block("<style>", "</style>")).map(|(span, code)| vec![(span, Node::Style(code))]),
			block("<!--", "-->").map(|_| Vec::new()),
			consumed(component).map(|(span, node)| vec![(span, node)]),
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
	let source = input;
	let (input, _) = char('{').parse(input)?;
	let (input, (code, _)) = cut(consumed(nom::multi::many_till(rust_fragment, peek(char('}'))))).parse(input)?;
	let (input, _) = cut(char('}')).parse(input)?;
	let span = &source[..source.len() - input.len()];

	Ok((input, (span, code.trim())))
}

fn directive(input: &str) -> Result<'_, (&str, SpannedNode<'_>)> {
	peek(tag("{#")).parse(input)?;
	consumed(cut(nom::branch::alt((conditional, for_loop)))).parse(input)
}

fn conditional(input: &str) -> Result<'_, (&str, Node<'_>)> {
	let (input, (_, _, condition, _)) = (tag("{#if"), multispace1, verify(take_until("}"), |condition: &str| !condition.trim().is_empty()), char('}')).parse(input)?;
	let (input, if_body) = nodes(input, None)?;
	let (input, (_, closing)) = reference(input)?;

	let (input, else_body) = match closing {
		"/if" => (input, None),
		":else" => {
			let (input, body) = nodes(input, None)?;
			let (input, _) = verify(reference, |(_, closing)| *closing == "/if").parse(input)?;

			(input, Some(body))
		}
		_ => return Err(nom::Err::Error(nom::error::Error::new(closing, nom::error::ErrorKind::Tag))),
	};

	Ok((
		input,
		Node::If {
			condition: condition.trim(),
			if_body,
			else_body,
		},
	))
}

fn for_loop(input: &str) -> Result<'_, (&str, Node<'_>)> {
	let (input, _) = (tag("{#for"), multispace1).parse(input)?;
	let (input, (pattern, _)) = consumed(nom::multi::many_till(rust_fragment, peek((multispace1, tag("in"), multispace1)))).parse(input)?;
	let (input, (_, _, _, items, _)) = (multispace1, tag("in"), multispace1, recognize(nom::multi::many0(rust_fragment)), char('}')).parse(input)?;
	let pattern = pattern.trim();
	let items = items.trim();

	if pattern.is_empty() || items.is_empty() {
		return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify)));
	}

	let (input, body) = nodes(input, None)?;
	let (input, _) = verify(reference, |(_, closing)| *closing == "/for").parse(input)?;

	Ok((input, Node::For { pattern, items, body }))
}

fn rust_fragment(input: &str) -> Result<'_, (&str, &str)> {
	nom::branch::alt((
		recognize((char('('), nom::multi::many0(rust_fragment), char(')'))),
		recognize((char('['), nom::multi::many0(rust_fragment), char(']'))),
		recognize((char('{'), nom::multi::many0(rust_fragment), char('}'))),
		recognize((char('"'), opt(escaped(is_not("\"\\"), '\\', anychar)), char('"'))),
		recognize((
			char('\''),
			nom::branch::alt((nom::sequence::preceded(char('\\'), anychar), satisfy(|c| c != '\\' && c != '\''))),
			char('\''),
		)),
		recognize(satisfy(|c| !matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '"'))),
	))
	.parse(input)
}

fn component(input: &str) -> Result<'_, (&str, Node<'_>)> {
	let (input, _) = char('<').parse(input)?;
	let (input, name) = recognize((
		satisfy(|character| character.is_ascii_uppercase()),
		take_while(|character: char| character.is_ascii_alphanumeric() || character == '_'),
	))
	.parse(input)?;
	let (input, props) = cut(nom::multi::many0(nom::sequence::preceded(multispace1, component_prop))).parse(input)?;
	let (input, (_, self_closing, _)) = cut((multispace0, opt(char('/')), char('>'))).parse(input)?;
	let (input, children) = if self_closing.is_some() {
		(input, Vec::new())
	} else {
		let (input, children) = nodes(input, Some(name))?;
		let (input, _) = component_end(input, name)?;
		(input, children)
	};

	Ok((input, Node::Component { name, props, children }))
}

fn component_end<'source>(input: &'source str, name: &str) -> Result<'source, (&'source str, ())> {
	let (input, _) = (tag("</"), tag(name), multispace0, char('>')).parse(input)?;

	Ok((input, ()))
}

fn component_prop(input: &str) -> Result<'_, (&str, ComponentProp<'_>)> {
	let (input, name) = take_while1(|character: char| character.is_ascii_alphanumeric() || character == '_').parse(input)?;
	let (input, _) = (multispace0, char('='), multispace0).parse(input)?;
	let (input, (_, value)) = reference(input)?;

	Ok((input, ComponentProp { name, value }))
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

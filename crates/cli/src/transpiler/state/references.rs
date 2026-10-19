use super::*;
use crate::*;

use std::collections;

pub struct StateReferences<'a> {
	pub states: &'a [declarations::StateDeclaration],
	pub references: Vec<syn::Ident>,
	pub shadowed: Vec<syn::Ident>,
	pub read_only: bool,
}

pub fn transpile_references(body: &mut syn::Block, states: &[declarations::StateDeclaration], updates: &[syn::Ident]) -> collections::BTreeMap<String, transpiler::function::FunctionReferences> {
	let mut function_references = collections::BTreeMap::new();

	for statement in &mut body.stmts {
		let syn::Stmt::Item(syn::Item::Fn(function)) = statement else {
			continue;
		};

		let ident = function.sig.ident.to_string();
		let mut references = StateReferences {
			states,
			references: Vec::new(),
			read_only: updates.contains(&function.sig.ident),
			shadowed: Vec::new(),
		};

		transform_function(&mut references, function);

		function_references.insert(
			ident,
			transpiler::function::FunctionReferences {
				states: references.references,
				callbacks: Vec::new(),
			},
		);
	}

	let mut update_states = Vec::new();

	for update in updates {
		let update_name = update.to_string();
		let Some(references) = function_references.get(&update_name) else {
			continue;
		};

		let mut mutable_states = Vec::new();
		for state in states {
			if state.is_mutable && references.states.contains(&state.ident) {
				mutable_states.push(state.ident.clone());
			}
		}

		update_states.push((update.clone(), mutable_states));
	}

	for (name, references) in &mut function_references {
		if updates.iter().any(|update| update.to_string() == *name) {
			continue;
		}

		for (update, mutable_states) in &update_states {
			if references.states.iter().any(|state| mutable_states.contains(state)) {
				references.callbacks.push(update.clone());
			}
		}
	}

	function_references
}

fn transform_function(references: &mut StateReferences<'_>, function: &mut syn::ItemFn) {
	let outer_scope = std::mem::take(&mut references.shadowed);

	for argument in &mut function.sig.inputs {
		if let syn::FnArg::Typed(argument) = argument {
			transform_pattern(references, &mut argument.pat);
		}
	}

	transform_block(references, &mut function.block);

	references.shadowed = outer_scope;
}

fn transform_block(references: &mut StateReferences<'_>, block: &mut syn::Block) {
	let scope_start = references.shadowed.len();

	for statement in &mut block.stmts {
		transform_statement(references, statement);
	}

	references.shadowed.truncate(scope_start);
}

fn transform_statement(references: &mut StateReferences<'_>, statement: &mut syn::Stmt) {
	match statement {
		syn::Stmt::Local(local) => {
			if let Some(init) = &mut local.init {
				transform_expression(references, &mut init.expr);

				if let Some((_, diverge)) = &mut init.diverge {
					transform_expression(references, diverge);
				}
			}
			transform_pattern(references, &mut local.pat);
		}
		syn::Stmt::Item(syn::Item::Fn(function)) => transform_function(references, function),
		syn::Stmt::Expr(expression, _) => transform_expression(references, expression),
		syn::Stmt::Macro(statement) => transform_macro(references, &mut statement.mac),
		_ => {}
	}
}

fn transform_pattern(references: &mut StateReferences<'_>, pattern: &mut syn::Pat) {
	match pattern {
		syn::Pat::Ident(pattern) => {
			references.shadowed.push(pattern.ident.clone());

			if let Some((_, subpattern)) = &mut pattern.subpat {
				transform_pattern(references, subpattern);
			}
		}
		syn::Pat::Guard(pattern) => {
			transform_pattern(references, &mut pattern.pat);
			transform_expression(references, &mut pattern.guard);
		}
		syn::Pat::Or(pattern) => {
			for case in &mut pattern.cases {
				transform_pattern(references, case);
			}
		}
		syn::Pat::Paren(pattern) => transform_pattern(references, &mut pattern.pat),
		syn::Pat::Reference(pattern) => transform_pattern(references, &mut pattern.pat),
		syn::Pat::Slice(pattern) => {
			for element in &mut pattern.elems {
				transform_pattern(references, element);
			}
		}
		syn::Pat::Struct(pattern) => {
			for field in &mut pattern.fields {
				transform_pattern(references, &mut field.pat);
			}
		}
		syn::Pat::Tuple(pattern) => {
			for element in &mut pattern.elems {
				transform_pattern(references, element);
			}
		}
		syn::Pat::TupleStruct(pattern) => {
			for element in &mut pattern.elems {
				transform_pattern(references, element);
			}
		}
		syn::Pat::Type(pattern) => transform_pattern(references, &mut pattern.pat),
		_ => {}
	}
}

fn transform_expression(references: &mut StateReferences<'_>, expression: &mut syn::Expr) {
	match expression {
		syn::Expr::Path(path) => {
			if let Some(state) = references
				.states
				.iter()
				.find(|state| path.qself.is_none() && path.path.is_ident(&state.ident) && !references.shadowed.contains(&state.ident))
			{
				if !references.references.contains(&state.ident) {
					references.references.push(state.ident.clone());
				}

				let attributes = &path.attrs;
				let name = &path.path;

				*expression = if state.is_mutable && !references.read_only {
					syn::parse_quote!(#(#attributes)* *#name.get_mut())
				} else {
					syn::parse_quote!(#(#attributes)* *#name.get())
				};
			}
		}
		syn::Expr::Array(expression) => {
			for element in &mut expression.elems {
				transform_expression(references, element);
			}
		}
		syn::Expr::Assign(expression) => {
			transform_expression(references, &mut expression.left);
			transform_expression(references, &mut expression.right);
		}
		syn::Expr::Async(expression) => transform_block(references, &mut expression.block),
		syn::Expr::Await(expression) => transform_expression(references, &mut expression.base),
		syn::Expr::Binary(expression) => {
			transform_expression(references, &mut expression.left);
			transform_expression(references, &mut expression.right);
		}
		syn::Expr::Block(expression) => transform_block(references, &mut expression.block),
		syn::Expr::Break(expression) => {
			if let Some(value) = &mut expression.expr {
				transform_expression(references, value);
			}
		}
		syn::Expr::Call(expression) => {
			transform_expression(references, &mut expression.func);
			for argument in &mut expression.args {
				transform_expression(references, argument);
			}
		}
		syn::Expr::Cast(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::Closure(expression) => {
			let scope_start = references.shadowed.len();
			for input in &mut expression.inputs {
				transform_pattern(references, input);
			}
			transform_expression(references, &mut expression.body);
			references.shadowed.truncate(scope_start);
		}
		syn::Expr::Const(expression) => transform_block(references, &mut expression.block),
		syn::Expr::Field(expression) => transform_expression(references, &mut expression.base),
		syn::Expr::ForLoop(expression) => {
			transform_expression(references, &mut expression.expr);

			let scope_start = references.shadowed.len();

			transform_pattern(references, &mut expression.pat);
			transform_block(references, &mut expression.body);

			references.shadowed.truncate(scope_start);
		}
		syn::Expr::Group(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::If(expression) => {
			let scope_start = references.shadowed.len();

			if let syn::Expr::Let(condition) = expression.cond.as_mut() {
				transform_expression(references, &mut condition.expr);
				transform_pattern(references, &mut condition.pat);
			} else {
				transform_expression(references, &mut expression.cond);
			}

			transform_block(references, &mut expression.then_branch);

			references.shadowed.truncate(scope_start);

			if let Some((_, else_branch)) = &mut expression.else_branch {
				transform_expression(references, else_branch);
			}
		}
		syn::Expr::Index(expression) => {
			transform_expression(references, &mut expression.expr);
			transform_expression(references, &mut expression.index);
		}
		syn::Expr::Let(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::Loop(expression) => transform_block(references, &mut expression.body),
		syn::Expr::Macro(expression) => transform_macro(references, &mut expression.mac),
		syn::Expr::Match(expression) => {
			transform_expression(references, &mut expression.expr);
			for arm in &mut expression.arms {
				let scope_start = references.shadowed.len();
				transform_pattern(references, &mut arm.pat);
				transform_expression(references, &mut arm.body);
				references.shadowed.truncate(scope_start);
			}
		}
		syn::Expr::MethodCall(expression) => {
			transform_expression(references, &mut expression.receiver);
			for argument in &mut expression.args {
				transform_expression(references, argument);
			}
		}
		syn::Expr::Paren(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::Range(expression) => {
			if let Some(start) = &mut expression.start {
				transform_expression(references, start);
			}
			if let Some(end) = &mut expression.end {
				transform_expression(references, end);
			}
		}
		syn::Expr::RawAddr(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::Reference(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::Repeat(expression) => {
			transform_expression(references, &mut expression.expr);
			transform_expression(references, &mut expression.len);
		}
		syn::Expr::Return(expression) => {
			if let Some(value) = &mut expression.expr {
				transform_expression(references, value);
			}
		}
		syn::Expr::Struct(expression) => {
			for field in &mut expression.fields {
				transform_expression(references, &mut field.expr);
				if field.colon_token.is_none() && !matches!(field.expr, syn::Expr::Path(_)) {
					field.colon_token = Some(Default::default());
				}
			}
			if let Some(rest) = &mut expression.rest {
				transform_expression(references, rest);
			}
		}
		syn::Expr::Try(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::TryBlock(expression) => transform_block(references, &mut expression.block),
		syn::Expr::Tuple(expression) => {
			for element in &mut expression.elems {
				transform_expression(references, element);
			}
		}
		syn::Expr::Unary(expression) => transform_expression(references, &mut expression.expr),
		syn::Expr::Unsafe(expression) => transform_block(references, &mut expression.block),
		syn::Expr::While(expression) => {
			let scope_start = references.shadowed.len();

			if let syn::Expr::Let(condition) = expression.cond.as_mut() {
				transform_expression(references, &mut condition.expr);
				transform_pattern(references, &mut condition.pat);
			} else {
				transform_expression(references, &mut expression.cond);
			}

			transform_block(references, &mut expression.body);
			references.shadowed.truncate(scope_start);
		}
		syn::Expr::Yield(expression) => {
			if let Some(value) = &mut expression.expr {
				transform_expression(references, value);
			}
		}
		_ => {}
	}
}

fn transform_macro(references: &mut StateReferences<'_>, invocation: &mut syn::Macro) {
	let Ok(mut arguments) = invocation.parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated) else {
		return;
	};

	for argument in &mut arguments {
		transform_expression(references, argument);
	}

	invocation.tokens = quote::quote!(#arguments);
}

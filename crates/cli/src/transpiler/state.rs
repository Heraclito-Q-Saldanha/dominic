use crate::*;

use std::collections;

use syn::visit_mut;

struct StateDeclaration {
	ident: syn::Ident,
	is_mutable: bool,
}

struct StateReferences<'a> {
	states: &'a [StateDeclaration],
	references: Vec<syn::Ident>,
	shadowed: Vec<syn::Ident>,
	read_only: bool,
}

impl visit_mut::VisitMut for StateReferences<'_> {
	fn visit_pat_ident_mut(&mut self, pattern: &mut syn::PatIdent) {
		self.shadowed.push(pattern.ident.clone());
		visit_mut::visit_pat_ident_mut(self, pattern);
	}

	fn visit_expr_mut(&mut self, expression: &mut syn::Expr) {
		if let syn::Expr::Path(path) = expression {
			if let Some(state) = self
				.states
				.iter()
				.find(|state| path.qself.is_none() && path.path.is_ident(&state.ident) && !self.shadowed.contains(&state.ident))
			{
				if !self.references.contains(&state.ident) {
					self.references.push(state.ident.clone());
				}

				let attributes = &path.attrs;
				let name = &path.path;

				*expression = if state.is_mutable && !self.read_only {
					syn::parse_quote!(#(#attributes)* *#name.get_mut())
				} else {
					syn::parse_quote!(#(#attributes)* *#name.get())
				};

				return;
			}
		}

		syn::visit_mut::visit_expr_mut(self, expression);
	}

	fn visit_block_mut(&mut self, block: &mut syn::Block) {
		let scope_start = self.shadowed.len();

		for statement in &mut block.stmts {
			self.visit_stmt_mut(statement);
		}

		self.shadowed.truncate(scope_start);
	}

	fn visit_local_mut(&mut self, local: &mut syn::Local) {
		self.visit_attributes_mut(&mut local.attrs);
		if let Some(init) = &mut local.init {
			self.visit_local_init_mut(init);
		}
		self.visit_pat_mut(&mut local.pat);
	}

	fn visit_item_fn_mut(&mut self, function: &mut syn::ItemFn) {
		let outer_scope = std::mem::take(&mut self.shadowed);
		visit_mut::visit_item_fn_mut(self, function);
		self.shadowed = outer_scope;
	}

	fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
		let scope_start = self.shadowed.len();
		visit_mut::visit_expr_closure_mut(self, closure);
		self.shadowed.truncate(scope_start);
	}

	fn visit_expr_if_mut(&mut self, expression: &mut syn::ExprIf) {
		let scope_start = self.shadowed.len();

		if let syn::Expr::Let(condition) = expression.cond.as_mut() {
			self.visit_expr_mut(condition.expr.as_mut());
			self.visit_pat_mut(condition.pat.as_mut());
		} else {
			self.visit_expr_mut(expression.cond.as_mut());
		}

		self.visit_block_mut(&mut expression.then_branch);
		self.shadowed.truncate(scope_start);

		if let Some((_, else_branch)) = &mut expression.else_branch {
			self.visit_expr_mut(else_branch);
		}
	}

	fn visit_expr_while_mut(&mut self, expression: &mut syn::ExprWhile) {
		let scope_start = self.shadowed.len();

		if let syn::Expr::Let(condition) = expression.cond.as_mut() {
			self.visit_expr_mut(condition.expr.as_mut());
			self.visit_pat_mut(condition.pat.as_mut());
		} else {
			self.visit_expr_mut(expression.cond.as_mut());
		}

		self.visit_block_mut(&mut expression.body);
		self.shadowed.truncate(scope_start);
	}

	fn visit_expr_for_loop_mut(&mut self, expression: &mut syn::ExprForLoop) {
		self.visit_expr_mut(expression.expr.as_mut());

		let scope_start = self.shadowed.len();
		self.visit_pat_mut(expression.pat.as_mut());
		self.visit_block_mut(&mut expression.body);
		self.shadowed.truncate(scope_start);
	}

	fn visit_arm_mut(&mut self, arm: &mut syn::Arm) {
		let scope_start = self.shadowed.len();
		self.visit_pat_mut(&mut arm.pat);
		self.visit_expr_mut(&mut arm.body);
		self.shadowed.truncate(scope_start);
	}

	fn visit_field_value_mut(&mut self, field: &mut syn::FieldValue) {
		syn::visit_mut::visit_field_value_mut(self, field);

		if field.colon_token.is_none() && !matches!(field.expr, syn::Expr::Path(_)) {
			field.colon_token = Some(Default::default());
		}
	}

	fn visit_macro_mut(&mut self, invocation: &mut syn::Macro) {
		let Ok(mut arguments) = invocation.parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated) else {
			return;
		};

		for argument in &mut arguments {
			self.visit_expr_mut(argument);
		}

		invocation.tokens = quote::quote!(#arguments);
	}
}

pub fn transpile_states(body: &mut syn::Block, updates: &[syn::Ident]) -> error::Result<collections::BTreeMap<String, transpiler::function::FunctionReferences>> {
	let mut states = Vec::new();

	for statement in &mut body.stmts {
		let syn::Stmt::Local(local) = statement else {
			continue;
		};

		let Some(init) = &mut local.init else {
			continue;
		};
		let syn::Expr::Macro(invocation) = init.expr.as_ref() else {
			continue;
		};
		if !invocation.mac.path.is_ident("state") {
			continue;
		}

		let value = invocation.mac.parse_body::<syn::Expr>()?;
		let attributes = &invocation.attrs;
		let pattern = match &mut local.pat {
			syn::Pat::Type(pattern) => {
				let value_type = pattern.ty.clone();
				pattern.ty = Box::new(syn::parse_quote!(::dominic_runtime::state::State<#value_type>));
				pattern.pat.as_mut()
			}
			pattern => pattern,
		};
		let syn::Pat::Ident(binding) = pattern else {
			return Err(syn::Error::new_spanned(&local.pat, "States need to be named bindings.").into());
		};

		states.push(StateDeclaration {
			ident: binding.ident.clone(),
			is_mutable: binding.mutability.take().is_some(),
		});

		init.expr = syn::parse_quote!(#(#attributes)* ::dominic_runtime::state::State::new(#value));
	}

	let mut function_references = collections::BTreeMap::new();

	for statement in &mut body.stmts {
		let mut visitor = StateReferences {
			states: &states,
			references: Vec::new(),
			read_only: matches!(statement, syn::Stmt::Item(syn::Item::Fn(function)) if updates.contains(&function.sig.ident)),
			shadowed: Vec::new(),
		};

		visit_mut::VisitMut::visit_stmt_mut(&mut visitor, statement);

		if let syn::Stmt::Item(syn::Item::Fn(function)) = statement {
			let ident = function.sig.ident.to_string();
			let references = transpiler::function::FunctionReferences {
				states: visitor.references,
				callbacks: Vec::new(),
			};

			function_references.insert(ident, references);
		}
	}

	let mut update_states = Vec::new();

	for update in updates {
		let update_name = update.to_string();
		let Some(references) = function_references.get(&update_name) else {
			continue;
		};

		let mut mutable_states = Vec::new();
		for state in &states {
			if state.is_mutable && references.states.contains(&state.ident) {
				mutable_states.push(state.ident.clone());
			}
		}

		update_states.push((update.clone(), mutable_states));
	}

	for (name, references) in &mut function_references {
		let is_update_function = updates.iter().any(|update| update.to_string() == *name);

		if is_update_function {
			continue;
		}

		for (update, mutable_states) in &update_states {
			let uses_mutable_state = references.states.iter().any(|state| mutable_states.contains(state));
			if uses_mutable_state {
				references.callbacks.push(update.clone());
			}
		}
	}

	Ok(function_references)
}

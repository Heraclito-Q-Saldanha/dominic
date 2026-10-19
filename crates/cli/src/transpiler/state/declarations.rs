use crate::*;

pub struct StateDeclaration {
	pub ident: syn::Ident,
	pub is_mutable: bool,
}

pub fn transpile_declarations(body: &mut syn::Block) -> error::Result<Vec<StateDeclaration>> {
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

	Ok(states)
}

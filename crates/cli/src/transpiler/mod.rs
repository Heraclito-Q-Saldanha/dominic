use crate::*;

use std::collections;
use std::mem;

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

pub fn transpile(input: &str) -> error::Result<(String, String)> {
	let (code, html) = split_code(input)?;

	let mut references = collections::BTreeMap::new();

	let mut html = lol_html::rewrite_str(
		&html,
		lol_html::RewriteStrSettings::new().append_element_content_handler(lol_html::element!("*", |element| {
			let mut attributes = Vec::new();

			for attribute in element.attributes() {
				let value = attribute.value();
				let Some(value) = value.trim().strip_prefix('{') else {
					continue;
				};
				let Some(value) = value.strip_suffix('}') else {
					continue;
				};
				let rs_name = value.trim();
				let js_name = format!("_{}_{}", rs_name, fastrand::u128(..));

				references.insert(rs_name.to_string(), js_name.clone());
				attributes.push((attribute.name(), format!("window.__dominic?.{js_name}()")));
			}

			for (name, value) in attributes {
				element.set_attribute(&name, &value)?;
			}

			Ok(())
		})),
	)?;

	let mut body = syn::parse_str::<syn::Block>(&format!("{{{code}}}"))?;

	for statement in mem::take(&mut body.stmts) {
		let syn::Stmt::Item(syn::Item::Fn(mut function)) = statement else {
			body.stmts.push(statement);
			continue;
		};

		let mut types = Vec::new();
		let mut arguments = Vec::new();

		for parameter in &function.sig.inputs {
			let syn::FnArg::Typed(parameter) = parameter else {
				return Err(syn::Error::new_spanned(&parameter, "Functions need to have typed parameters.").into());
			};
			let syn::Pat::Ident(argument) = parameter.pat.as_ref() else {
				return Err(syn::Error::new_spanned(&parameter.pat, "Function parameters need to be named bindings.").into());
			};

			types.push(&parameter.ty);
			arguments.push(&argument.ident);
		}

		let rs_name = function.sig.ident.to_string();

		if let Some(js_name) = references.get(&rs_name) {
			function.attrs.push(syn::parse_quote! {
				#[dominic_runtime::bindgen(js_name = #js_name, wasm_bindgen = dominic_runtime::wasm_bindgen)]
			});
			function.vis = syn::parse_quote!(pub);
		}

		let function_block = &function.block;
		let closure_body = match function.sig.safety {
			syn::Safety::Unsafe(_) => quote::quote!({ unsafe #function_block }),
			_ => quote::quote!(#function_block),
		};

		let uninitialized_error = format!("function `{rs_name}` called before initialization");
		let storage = quote::format_ident!("__{}_closure_{}", &rs_name, fastrand::u128(..));

		function.block = syn::parse_quote!({
			#storage.with_borrow_mut(|closure| {
				closure.as_mut().expect(#uninitialized_error)(#(#arguments),*)
			})
		});

		let parameters = &function.sig.inputs;
		let output = &function.sig.output;

		let replacement: syn::Block = syn::parse_quote!({
			::std::thread_local! {
				#[allow(non_upper_case_globals)]
				static #storage: ::std::cell::RefCell<::core::option::Option<::std::boxed::Box<dyn ::std::ops::FnMut(#(#types),*) #output>>> =
					const { ::std::cell::RefCell::new(::core::option::Option::None) };
			}

			#function

			#storage.set(::core::option::Option::Some(::std::boxed::Box::new(move |#parameters| #output #closure_body)));
		});

		body.stmts.extend(replacement.stmts);
	}

	let init_fn = quote::format_ident!("__init_{:032x}", fastrand::u128(..));

	let code = quote::quote! {
		#[dominic_runtime::bindgen(wasm_bindgen = dominic_runtime::wasm_bindgen)]
		pub fn #init_fn() #body
	}
	.to_string();

	let script = format!(
		r#"
<script type="module">
	import * as script from "/index.js";

	const start = async () => {{
		await script.default({{ module_or_path: "/index_bg.wasm" }});
		window.__dominic = script;
		window.__dominic.{init_fn}();
	}};
	if (document.readyState === "complete") {{
		start();
	}} else {{
		window.addEventListener("load", start, {{ once: true }});
	}}
</script>
"#
	);

	html.push_str(&script);

	Ok((code, html))
}

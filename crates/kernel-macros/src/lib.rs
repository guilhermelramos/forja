//! Attribute macro that publishes a module factory into
//! [`forja_kernel::MODULE_FACTORIES`].
//!
//! Usage in a module crate:
//!
//! ```ignore
//! #[forja_kernel::module]
//! static HELLO: fn() -> Box<dyn forja_kernel::Module> = || Box::new(Hello);
//! ```
//!
//! The macro prepends `#[linkme::distributed_slice(...)]` and also exports
//! `__forja_module()`, a direct constructor used by the shell's build script
//! on toolchains where the linker does not gather distributed slices (the
//! Windows GNU target is the known case). One module per crate.

use proc_macro::{TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let Some(static_name) = static_identifier(&item) else {
        return "compile_error!(\"forja_kernel::module must be applied to a static\");"
            .parse()
            .unwrap_or_default();
    };
    let body = item.to_string();
    let expanded = format!(
        "#[::linkme::distributed_slice(::forja_kernel::MODULE_FACTORIES)]\n\
         {body}\n\
         #[doc(hidden)]\n\
         pub fn __forja_module() -> ::std::boxed::Box<dyn ::forja_kernel::Module> {{ {static_name}() }}"
    );
    match expanded.parse() {
        Ok(tokens) => tokens,
        Err(error) => {
            let message = format!("forja-kernel module macro produced invalid tokens: {error}");
            format!("compile_error!({message:?});").parse().unwrap_or_default()
        }
    }
}

fn static_identifier(item: &TokenStream) -> Option<String> {
    let mut tokens = item.clone().into_iter();
    while let Some(token) = tokens.next() {
        if let TokenTree::Ident(ident) = token {
            if ident.to_string() == "static" {
                if let Some(TokenTree::Ident(name)) = tokens.next() {
                    return Some(name.to_string());
                }
                return None;
            }
        }
    }
    None
}

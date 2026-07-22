//! `#[retros::main]` (plans.md §5.1).
//!
//! A RetrOS wasm app has to export `tick` (and optionally `init` / `on_event`) with the
//! exact C signatures the host imports. Writing `#[no_mangle] pub extern "C"` by hand on
//! each is easy to get subtly wrong, so this macro does it: an author writes an ordinary
//! Rust function and the macro emits the export shim around it.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, ReturnType};

/// Mark an entry point so the engine can find it.
///
/// ```ignore
/// #[retros::main]
/// fn init() { retros::set_title("Hello"); }
///
/// #[retros::main]
/// fn tick(dt: f32) { retros::clear(retros::rgb(0, 0, 0)); }
/// ```
///
/// The function must be named `init`, `tick`, `on_event` or `shutdown`, since those are
/// the names the host looks up.
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let name = function.sig.ident.clone();
    let name_str = name.to_string();

    let known = ["init", "tick", "on_event", "shutdown"];
    if !known.contains(&name_str.as_str()) {
        return syn::Error::new_spanned(
            &function.sig.ident,
            format!(
                "#[retros::main] expects a function named one of: {}. \
                 The host looks these up by name in the wasm module's exports.",
                known.join(", ")
            ),
        )
        .to_compile_error()
        .into();
    }

    if !matches!(function.sig.output, ReturnType::Default) {
        return syn::Error::new_spanned(
            &function.sig.output,
            "#[retros::main] entry points must not return a value",
        )
        .to_compile_error()
        .into();
    }

    let inner_name = syn::Ident::new(&format!("__retros_inner_{name_str}"), name.span());
    let mut inner = function.clone();
    inner.sig.ident = inner_name.clone();

    // Rebuild the parameter list for the exported shim, so the export carries the exact
    // types the caller declared rather than something guessed.
    let params: Vec<_> = function
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(|(i, arg)| match arg {
            FnArg::Typed(t) => {
                let ident = syn::Ident::new(&format!("arg{i}"), t.pat.span_hint());
                let ty = &t.ty;
                quote!(#ident: #ty)
            }
            FnArg::Receiver(r) => syn::Error::new_spanned(
                r,
                "#[retros::main] cannot be used on a method",
            )
            .to_compile_error(),
        })
        .collect();

    let args: Vec<_> = (0..function.sig.inputs.len())
        .map(|i| syn::Ident::new(&format!("arg{i}"), proc_macro2::Span::call_site()))
        .collect();

    quote! {
        #inner

        #[no_mangle]
        pub extern "C" fn #name(#(#params),*) {
            #inner_name(#(#args),*);
        }
    }
    .into()
}

/// Small helper so the parameter rebuild above stays readable.
trait SpanHint {
    fn span_hint(&self) -> proc_macro2::Span;
}

impl SpanHint for Box<syn::Pat> {
    fn span_hint(&self) -> proc_macro2::Span {
        use syn::spanned::Spanned;
        self.span()
    }
}

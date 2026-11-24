//! Procedural macros for [`octlui`](https://github.com/ooctlot/octlui).
//!
//! * [`action`] turns a struct/enum into an octlui `Action`.
//! * [`builder`] + `#[property]` generate a fluent builder API for a struct.

mod action;
mod builder;

use proc_macro::TokenStream;

/// Marks a struct or enum as an action.
///
/// Adds `Clone` + `Debug` derives when they are missing and implements
/// `octlui::Action` and `octlui::Event` for the type.
///
/// ```ignore
/// #[octlui::action]
/// struct Reset;
///
/// #[octlui::action]
/// struct Change { amount: i32 }
/// ```
#[proc_macro_attribute]
pub fn action(attr: TokenStream, item: TokenStream) -> TokenStream {
    action::expand(attr.into(), item.into()).into()
}

/// Generates a constructor and fluent setters for a struct.
///
/// * The constructor is named after the struct in `snake_case`
///   (`Node` -> `Node::node()`) and uses `Default`.
///   `#[octlui::builder(free)]` additionally emits a free function with the same name,
///   `#[octlui::builder(no_constructor)]` skips the constructor and
///   `#[octlui::builder(name = "new")]` renames it.
/// * Every field annotated with `#[octlui::property]` gets a by-value setter
///   `fn field(self, value) -> Self`. `Option<T>` fields take a `T` and store `Some(value)`.
///
/// Property options: `into` (accept `impl Into<T>`), `boxed` (field is `Option<Box<T>>`,
/// the setter takes `T`), `name = "other"` (rename the setter).
#[proc_macro_attribute]
pub fn builder(attr: TokenStream, item: TokenStream) -> TokenStream {
    builder::expand(attr.into(), item.into()).into()
}

/// Marker attribute consumed by [`builder`]. It is stripped during expansion; using it on a
/// struct that is not annotated with `#[octlui::builder]` is an error.
#[proc_macro_attribute]
pub fn property(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let msg = "`#[octlui::property]` can only be used on fields of a struct annotated with `#[octlui::builder]`";
    let err: TokenStream = syn::Error::new(proc_macro2::Span::call_site(), msg)
        .to_compile_error()
        .into();
    err.into_iter().chain(item).collect()
}

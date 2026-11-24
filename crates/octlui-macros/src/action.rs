use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Item, parse2};

const WANTED_DERIVES: [&str; 2] = ["Clone", "Debug"];

fn existing_derives(attrs: &[Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("derive")) {
        let _ = attr.parse_nested_meta(|meta| {
            if let Some(seg) = meta.path.segments.last() {
                out.push(seg.ident.to_string());
            }
            Ok(())
        });
    }
    out
}

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item: Item = match parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };
    let (attrs, ident, generics) = match &mut item {
        Item::Struct(s) => (&mut s.attrs, s.ident.clone(), s.generics.clone()),
        Item::Enum(e) => (&mut e.attrs, e.ident.clone(), e.generics.clone()),
        other => {
            return syn::Error::new_spanned(
                other,
                "`#[octlui::action]` supports structs and enums",
            )
            .to_compile_error();
        }
    };

    let have = existing_derives(attrs);
    let missing: Vec<_> = WANTED_DERIVES
        .iter()
        .filter(|d| !have.iter().any(|h| h == *d))
        .map(|d| syn::Ident::new(d, proc_macro2::Span::call_site()))
        .collect();
    if !missing.is_empty() {
        attrs.insert(0, syn::parse_quote!(#[derive(#(#missing),*)]));
    }

    let (impl_g, ty_g, where_g) = generics.split_for_impl();
    let name = ident.to_string();
    quote! {
        #item

        impl #impl_g ::octlui::Action for #ident #ty_g #where_g {
            fn name(&self) -> &'static str { concat!(module_path!(), "::", #name) }
            fn as_any(&self) -> &dyn ::core::any::Any { self }
            fn boxed_clone(&self) -> ::std::boxed::Box<dyn ::octlui::Action> {
                ::std::boxed::Box::new(::core::clone::Clone::clone(self))
            }
        }
        impl #impl_g ::octlui::Event for #ident #ty_g #where_g {}
    }
}

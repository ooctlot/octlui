use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Attribute, Field, GenericArgument, Ident, ItemStruct, LitStr, PathArguments, Type, parse2,
};

#[derive(Default)]
struct BuilderArgs {
    free: bool,
    no_constructor: bool,
    name: Option<String>,
}

#[derive(Default)]
struct PropertyArgs {
    into: bool,
    boxed: bool,
    name: Option<String>,
}

fn is_property(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|s| s.ident == "property")
}

fn parse_property(attr: &Attribute) -> syn::Result<PropertyArgs> {
    let mut args = PropertyArgs::default();
    if matches!(attr.meta, syn::Meta::Path(_)) {
        return Ok(args);
    }
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("into") {
            args.into = true;
        } else if meta.path.is_ident("boxed") {
            args.boxed = true;
        } else if meta.path.is_ident("name") {
            let lit: LitStr = meta.value()?.parse()?;
            args.name = Some(lit.value());
        } else {
            return Err(meta.error("unknown property option (expected `into`, `boxed` or `name`)"));
        }
        Ok(())
    })?;
    Ok(args)
}

/// If `ty` is `Name<Inner>` returns `Inner`.
fn unwrap_generic<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    match args.args.first()? {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    }
}

fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn setter(field: &Field, args: &PropertyArgs) -> syn::Result<TokenStream> {
    let field_ident = field
        .ident
        .clone()
        .ok_or_else(|| syn::Error::new_spanned(field, "`#[property]` requires a named field"))?;
    let fn_ident = Ident::new(
        args.name.as_deref().unwrap_or(&field_ident.to_string()),
        Span::call_site(),
    );
    let docs: Vec<_> = field
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .collect();
    let ty = &field.ty;

    // Work out the user-facing value type and how to store it.
    let (value_ty, store): (TokenStream, Box<dyn Fn(TokenStream) -> TokenStream>) =
        if let Some(inner) = unwrap_generic(ty, "Option") {
            if args.boxed {
                let Some(boxed_inner) = unwrap_generic(inner, "Box") else {
                    return Err(syn::Error::new_spanned(
                        ty,
                        "`boxed` properties must have the type `Option<Box<T>>`",
                    ));
                };
                (
                    quote!(#boxed_inner),
                    Box::new(|v| quote!(::core::option::Option::Some(::std::boxed::Box::new(#v)))),
                )
            } else {
                (
                    quote!(#inner),
                    Box::new(|v| quote!(::core::option::Option::Some(#v))),
                )
            }
        } else {
            if args.boxed {
                return Err(syn::Error::new_spanned(
                    ty,
                    "`boxed` requires an `Option<Box<T>>` field",
                ));
            }
            (quote!(#ty), Box::new(|v| v))
        };

    let (param_ty, conv) = if args.into {
        (
            quote!(impl ::core::convert::Into<#value_ty>),
            quote!(::core::convert::Into::into(value)),
        )
    } else {
        (value_ty, quote!(value))
    };
    let stored = store(conv);
    Ok(quote! {
        #(#docs)*
        #[inline]
        #[must_use]
        pub fn #fn_ident(mut self, value: #param_ty) -> Self {
            self.#field_ident = #stored;
            self
        }
    })
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut args = BuilderArgs::default();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("free") {
            args.free = true;
        } else if meta.path.is_ident("no_constructor") {
            args.no_constructor = true;
        } else if meta.path.is_ident("name") {
            let lit: LitStr = meta.value()?.parse()?;
            args.name = Some(lit.value());
        } else {
            return Err(
                meta.error("unknown builder option (expected `free`, `no_constructor` or `name`)")
            );
        }
        Ok(())
    });
    if let Err(e) = syn::parse::Parser::parse2(parser, attr) {
        return e.to_compile_error();
    }

    let mut item: ItemStruct = match parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let mut setters = Vec::new();
    let mut errors = TokenStream::new();
    for field in item.fields.iter_mut() {
        let prop_attrs: Vec<Attribute> = field
            .attrs
            .iter()
            .filter(|a| is_property(a))
            .cloned()
            .collect();
        if prop_attrs.is_empty() {
            continue;
        }
        field.attrs.retain(|a| !is_property(a));
        let parsed = parse_property(&prop_attrs[0]).and_then(|a| setter(field, &a));
        match parsed {
            Ok(ts) => setters.push(ts),
            Err(e) => errors.extend(e.to_compile_error()),
        }
    }

    let ident = &item.ident;
    let (impl_g, ty_g, where_g) = item.generics.split_for_impl();
    let ctor_name = args
        .name
        .clone()
        .unwrap_or_else(|| to_snake(&ident.to_string()));
    let ctor_ident = format_ident!("{}", ctor_name);

    let ctor = if args.no_constructor {
        quote!()
    } else {
        quote! {
            #[doc = concat!("Creates a new [`", stringify!(#ident), "`] with default values.")]
            #[inline]
            #[must_use]
            pub fn #ctor_ident() -> Self { <Self as ::core::default::Default>::default() }
        }
    };
    let free_fn = if args.free && !args.no_constructor && item.generics.params.is_empty() {
        quote! {
            #[doc = concat!("Creates a new [`", stringify!(#ident), "`] with default values.")]
            #[inline]
            #[must_use]
            pub fn #ctor_ident() -> #ident { <#ident as ::core::default::Default>::default() }
        }
    } else {
        quote!()
    };

    quote! {
        #item
        #errors
        impl #impl_g #ident #ty_g #where_g {
            #ctor
            #(#setters)*
        }
        #free_fn
    }
}

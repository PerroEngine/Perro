use proc_macro::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::{Expr, Field, Fields, ItemStruct, Meta, Result, parse::Parse, parse_macro_input, parse_quote};

struct EmptyAttrArgs;

impl Parse for EmptyAttrArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        if input.is_empty() {
            Ok(Self)
        } else {
            Err(input.error("`State` does not accept arguments"))
        }
    }
}

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn State(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Err(err) = syn::parse::<EmptyAttrArgs>(attr) {
        return err.into_compile_error().into();
    }

    let mut item_struct = parse_macro_input!(item as ItemStruct);
    let default_init = match build_default_initializer(&mut item_struct) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error().into(),
    };

    let struct_ident = &item_struct.ident;
    let generics = &item_struct.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        #item_struct

        impl #impl_generics ::core::default::Default for #struct_ident #ty_generics #where_clause {
            fn default() -> Self {
                #default_init
            }
        }
    };
    expanded.into()
}

fn build_default_initializer(item_struct: &mut ItemStruct) -> Result<proc_macro2::TokenStream> {
    match &mut item_struct.fields {
        Fields::Named(fields) => {
            let mut inits = Vec::with_capacity(fields.named.len());
            for field in &mut fields.named {
                let ident = field
                    .ident
                    .as_ref()
                    .expect("named fields always have an ident")
                    .clone();
                let value = take_default_expr(field)?
                    .unwrap_or_else(|| parse_quote!(::core::default::Default::default()));
                inits.push(quote! { #ident: #value });
            }
            Ok(quote! { Self { #(#inits,)* } })
        }
        Fields::Unnamed(fields) => {
            let mut inits = Vec::with_capacity(fields.unnamed.len());
            for field in &mut fields.unnamed {
                let value = take_default_expr(field)?
                    .unwrap_or_else(|| parse_quote!(::core::default::Default::default()));
                inits.push(value);
            }
            Ok(quote! { Self( #(#inits,)* ) })
        }
        Fields::Unit => Ok(quote! { Self }),
    }
}

fn take_default_expr(field: &mut Field) -> Result<Option<Expr>> {
    let mut default_expr: Option<Expr> = None;
    let mut retained = Vec::with_capacity(field.attrs.len());

    for attr in field.attrs.drain(..) {
        if !attr.path().is_ident("default") {
            retained.push(attr);
            continue;
        }

        if default_expr.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate `default` attribute on field",
            ));
        }

        default_expr = Some(parse_default_expr(&attr.meta)?);
    }

    field.attrs = retained;
    Ok(default_expr)
}

fn parse_default_expr(meta: &Meta) -> Result<Expr> {
    match meta {
        Meta::NameValue(named) => syn::parse2(named.value.to_token_stream()),
        Meta::List(list) => list.parse_args::<Expr>(),
        Meta::Path(path) => Err(syn::Error::new_spanned(
            path,
            "`default` requires an expression, for example `#[default = 5]`",
        )),
    }
}

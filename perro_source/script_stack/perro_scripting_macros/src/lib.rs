use proc_macro::TokenStream;
use quote::ToTokens;
use quote::quote;
use syn::{
    Data, DeriveInput, Expr, Field, Fields, ItemStruct, Meta, Result, Variant, parse::Parse,
    parse_macro_input, parse_quote,
};

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

#[proc_macro_derive(Variant)]
pub fn derive_variant(input: TokenStream) -> TokenStream {
    derive_variant_like(input)
}

#[proc_macro_derive(DeriveVariant)]
pub fn derive_variant_codec(input: TokenStream) -> TokenStream {
    derive_variant_like(input)
}

fn derive_variant_like(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match input.data {
        Data::Struct(data_struct) => derive_state_field_struct(
            ident,
            impl_generics,
            ty_generics,
            where_clause,
            data_struct.fields,
        ),
        Data::Enum(data_enum) => derive_state_field_enum(
            ident,
            impl_generics,
            ty_generics,
            where_clause,
            data_enum.variants.into_iter().collect(),
        ),
        _ => syn::Error::new_spanned(
            ident,
            "`Variant` derive only supports structs with named fields or enums",
        )
        .into_compile_error()
        .into(),
    }
}

fn derive_state_field_struct(
    ident: syn::Ident,
    impl_generics: syn::ImplGenerics<'_>,
    ty_generics: syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
    fields: Fields,
) -> TokenStream {
    let Fields::Named(fields) = fields else {
        return syn::Error::new_spanned(
            ident,
            "`Variant` derive on structs only supports named fields",
        )
        .into_compile_error()
        .into();
    };

    let mut from_fields = Vec::new();
    let mut to_fields = Vec::new();
    let mut schema_fields = Vec::new();
    let mut codec_hints = Vec::new();

    for field in fields.named {
        let Some(field_ident) = field.ident else {
            continue;
        };
        let field_ty = field.ty;
        let field_key = field_ident.to_string();
        schema_fields.push(field_key.clone());
        codec_hints.push(quote! {
            __perro_hint_use_derive_variant_or_derive_variantcodec::<#field_ty>();
        });

        from_fields.push(quote! {
            #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(obj.get(#field_key)?)?
        });
        to_fields.push(quote! {
            out.insert(::std::sync::Arc::<str>::from(#field_key), ::perro_api::variant::DeriveVariant::to_variant(&self.#field_ident));
        });
    }

    let expanded = quote! {
        impl #impl_generics ::perro_api::variant::DeriveVariant for #ident #ty_generics #where_clause {
            fn from_variant(value: &::perro_api::variant::Variant) -> ::core::option::Option<Self> {
                fn __perro_hint_use_derive_variant_or_derive_variantcodec<T: ::perro_api::variant::DeriveVariant>() {}
                #(#codec_hints)*
                let obj = value.as_object()?;
                Some(Self {
                    #(#from_fields,)*
                })
            }

            fn to_variant(&self) -> ::perro_api::variant::Variant {
                let mut out = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                #(#to_fields)*
                ::perro_api::variant::Variant::Object(out)
            }
        }

        impl #impl_generics ::perro_api::variant::VariantSchema for #ident #ty_generics #where_clause {
            fn field_names() -> &'static [&'static str] {
                &[#(#schema_fields),*]
            }
        }

        impl #impl_generics ::core::convert::From<#ident #ty_generics> for ::perro_api::variant::Variant #where_clause {
            fn from(value: #ident #ty_generics) -> Self {
                ::perro_api::variant::DeriveVariant::to_variant(&value)
            }
        }
    };

    expanded.into()
}

fn derive_state_field_enum(
    ident: syn::Ident,
    impl_generics: syn::ImplGenerics<'_>,
    ty_generics: syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
    variants: Vec<Variant>,
) -> TokenStream {
    let mut from_arms = Vec::new();
    let mut to_arms = Vec::new();
    let mut codec_hints = Vec::new();

    for variant in variants {
        let variant_ident = variant.ident;
        let variant_name = variant_ident.to_string();
        match variant.fields {
            Fields::Unit => {
                from_arms.push(quote! {
                    #variant_name => Some(Self::#variant_ident),
                });
                to_arms.push(quote! {
                    Self::#variant_ident => {
                        out.insert(
                            ::std::sync::Arc::<str>::from("__variant"),
                            ::perro_api::variant::Variant::String(::std::sync::Arc::<str>::from(#variant_name)),
                        );
                    }
                });
            }
            Fields::Unnamed(fields) => {
                let mut from_values = Vec::new();
                let mut to_values = Vec::new();
                let mut bindings = Vec::new();
                let expected_len = fields.unnamed.len();

                for (idx, field) in fields.unnamed.into_iter().enumerate() {
                    let field_ty = field.ty;
                    codec_hints.push(quote! {
                        __perro_hint_use_derive_variant_or_derive_variantcodec::<#field_ty>();
                    });
                    let binding = syn::Ident::new(
                        &format!("__perro_v{}", idx),
                        proc_macro2::Span::call_site(),
                    );
                    let index = syn::Index::from(idx);
                    from_values.push(quote! {
                        <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(data.get(#index)?)?
                    });
                    to_values.push(quote! {
                        ::perro_api::variant::DeriveVariant::to_variant(#binding)
                    });
                    bindings.push(binding);
                }

                to_arms.push(quote! {
                    Self::#variant_ident( #( #bindings ),* ) => {
                        out.insert(
                            ::std::sync::Arc::<str>::from("__variant"),
                            ::perro_api::variant::Variant::String(::std::sync::Arc::<str>::from(#variant_name)),
                        );
                        let data = vec![#(#to_values),*];
                        out.insert(
                            ::std::sync::Arc::<str>::from("__data"),
                            ::perro_api::variant::Variant::Array(data),
                        );
                    }
                });

                from_arms.push(quote! {
                    #variant_name => {
                        let data = obj.get("__data")?.as_array()?;
                        if data.len() != #expected_len {
                            return None;
                        }
                        Some(Self::#variant_ident( #(#from_values),* ))
                    }
                });
            }
            Fields::Named(fields) => {
                let mut from_fields = Vec::new();
                let mut to_fields = Vec::new();
                let mut bindings = Vec::new();

                for field in fields.named {
                    let Some(field_ident) = field.ident else {
                        continue;
                    };
                    let field_ty = field.ty;
                    codec_hints.push(quote! {
                        __perro_hint_use_derive_variant_or_derive_variantcodec::<#field_ty>();
                    });
                    let key = field_ident.to_string();
                    from_fields.push(quote! {
                        #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(data.get(#key)?)?
                    });
                    to_fields.push(quote! {
                        data.insert(
                            ::std::sync::Arc::<str>::from(#key),
                            ::perro_api::variant::DeriveVariant::to_variant(#field_ident),
                        );
                    });
                    bindings.push(field_ident);
                }

                to_arms.push(quote! {
                    Self::#variant_ident { #( #bindings ),* } => {
                        out.insert(
                            ::std::sync::Arc::<str>::from("__variant"),
                            ::perro_api::variant::Variant::String(::std::sync::Arc::<str>::from(#variant_name)),
                        );
                        let mut data = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                        #(#to_fields)*
                        out.insert(
                            ::std::sync::Arc::<str>::from("__data"),
                            ::perro_api::variant::Variant::Object(data),
                        );
                    }
                });

                from_arms.push(quote! {
                    #variant_name => {
                        let data = obj.get("__data")?.as_object()?;
                        Some(Self::#variant_ident {
                            #(#from_fields),*
                        })
                    }
                });
            }
        }
    }

    let expanded = quote! {
        impl #impl_generics ::perro_api::variant::DeriveVariant for #ident #ty_generics #where_clause {
            fn from_variant(value: &::perro_api::variant::Variant) -> ::core::option::Option<Self> {
                fn __perro_hint_use_derive_variant_or_derive_variantcodec<T: ::perro_api::variant::DeriveVariant>() {}
                #(#codec_hints)*
                let obj = value.as_object()?;
                let tag = obj.get("__variant")?.as_str()?;
                match tag {
                    #(#from_arms)*
                    _ => None,
                }
            }

            fn to_variant(&self) -> ::perro_api::variant::Variant {
                let mut out = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                match self {
                    #(#to_arms),*
                }
                ::perro_api::variant::Variant::Object(out)
            }
        }

        impl #impl_generics ::perro_api::variant::VariantSchema for #ident #ty_generics #where_clause {}

        impl #impl_generics ::core::convert::From<#ident #ty_generics> for ::perro_api::variant::Variant #where_clause {
            fn from(value: #ident #ty_generics) -> Self {
                ::perro_api::variant::DeriveVariant::to_variant(&value)
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

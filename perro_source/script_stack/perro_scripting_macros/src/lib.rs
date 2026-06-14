use proc_macro::TokenStream;
use quote::ToTokens;
use quote::quote;
use syn::{
    Data, DeriveInput, Expr, Field, Fields, GenericParam, Generics, ItemStruct, LitStr, Meta,
    Result, Variant, parse::Parse, parse_macro_input, parse_quote,
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

#[proc_macro_derive(Variant, attributes(variant, node_ref))]
pub fn derive_variant(input: TokenStream) -> TokenStream {
    derive_variant_like(input)
}

#[proc_macro_derive(DeriveVariant, attributes(variant, node_ref))]
pub fn derive_variant_codec(input: TokenStream) -> TokenStream {
    derive_variant_like(input)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StructMode {
    Object,
    Array,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnumTagMode {
    String,
    U16,
}

#[derive(Clone, Copy)]
struct VariantDeriveOptions {
    struct_mode: StructMode,
    enum_tag_mode: EnumTagMode,
}

impl Default for VariantDeriveOptions {
    fn default() -> Self {
        Self {
            struct_mode: StructMode::Array,
            enum_tag_mode: EnumTagMode::U16,
        }
    }
}

fn derive_variant_like(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let options = match parse_variant_derive_options(&input) {
        Ok(v) => v,
        Err(err) => return err.into_compile_error().into(),
    };
    let ident = input.ident;
    let generics = add_derive_variant_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match input.data {
        Data::Struct(data_struct) => derive_state_field_struct(
            ident,
            impl_generics,
            ty_generics,
            where_clause,
            data_struct.fields,
            options,
        ),
        Data::Enum(data_enum) => derive_state_field_enum(
            ident,
            impl_generics,
            ty_generics,
            where_clause,
            data_enum.variants.into_iter().collect(),
            options,
        ),
        _ => syn::Error::new_spanned(ident, "`Variant` derive only supports structs or enums")
            .into_compile_error()
            .into(),
    }
}

fn add_derive_variant_bounds(mut generics: Generics) -> Generics {
    for param in generics.params.iter_mut() {
        let GenericParam::Type(type_param) = param else {
            continue;
        };
        type_param
            .bounds
            .push(parse_quote!(::perro_api::variant::DeriveVariant));
    }
    generics
}

fn parse_variant_derive_options(input: &DeriveInput) -> Result<VariantDeriveOptions> {
    let mut options = VariantDeriveOptions::default();
    for attr in &input.attrs {
        if !attr.path().is_ident("variant") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("mode") {
                let lit: LitStr = meta.value()?.parse()?;
                match lit.value().as_str() {
                    "object" => options.struct_mode = StructMode::Object,
                    "array" => options.struct_mode = StructMode::Array,
                    _ => {
                        return Err(meta.error(
                            "`variant(mode = ...)` only supports `\"object\"` or `\"array\"`",
                        ));
                    }
                }
                return Ok(());
            }
            if meta.path.is_ident("tag") {
                let lit: LitStr = meta.value()?.parse()?;
                match lit.value().as_str() {
                    "string" => options.enum_tag_mode = EnumTagMode::String,
                    "u16" => options.enum_tag_mode = EnumTagMode::U16,
                    _ => {
                        return Err(meta.error(
                            "`variant(tag = ...)` only supports `\"string\"` or `\"u16\"`",
                        ));
                    }
                }
                return Ok(());
            }
            Err(meta.error("unknown `variant` option; use `mode = ...` or `tag = ...`"))
        })?;
    }
    Ok(options)
}

fn shared_arc_str(value: &str) -> proc_macro2::TokenStream {
    quote! {{
        static __PERRO_VARIANT_KEY: ::std::sync::LazyLock<::std::sync::Arc<str>> =
            ::std::sync::LazyLock::new(|| ::std::sync::Arc::<str>::from(#value));
        ::std::sync::Arc::clone(&__PERRO_VARIANT_KEY)
    }}
}

fn derive_state_field_struct(
    ident: syn::Ident,
    impl_generics: syn::ImplGenerics<'_>,
    ty_generics: syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
    fields: Fields,
    options: VariantDeriveOptions,
) -> TokenStream {
    let mut from_fields = Vec::new();
    let mut from_owned_fields = Vec::new();
    let mut to_fields = Vec::new();
    let mut into_fields = Vec::new();
    let mut named_field_idents = Vec::new();
    let mut tuple_field_idents = Vec::new();
    let mut schema_fields = Vec::new();
    let mut codec_hints = Vec::new();
    let mut struct_is_tuple = false;
    let mut struct_is_unit = false;

    match fields {
        Fields::Named(fields) => {
            for field in fields.named {
                let Some(field_ident) = field.ident else {
                    continue;
                };
                let field_ty = field.ty;
                let field_key = field_ident.to_string();
                let field_key_arc = shared_arc_str(&field_key);
                named_field_idents.push(field_ident.clone());
                schema_fields.push(field_key.clone());
                codec_hints.push(quote! {
                    __perro_hint_use_derive_variant::<#field_ty>();
                });

                match options.struct_mode {
                    StructMode::Object => {
                        from_fields.push(quote! {
                            #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(obj.get(#field_key)?)?
                        });
                        from_owned_fields.push(quote! {
                            #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_owned_variant(obj.remove(#field_key)?)?
                        });
                        to_fields.push(quote! {
                            out.insert(#field_key_arc, ::perro_api::variant::DeriveVariant::to_variant(&self.#field_ident));
                        });
                        into_fields.push(quote! {
                            out.insert(#field_key_arc, ::perro_api::variant::DeriveVariant::into_variant(#field_ident));
                        });
                    }
                    StructMode::Array => {
                        let idx = syn::Index::from(from_fields.len());
                        from_fields.push(quote! {
                            #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(data.get(#idx)?)?
                        });
                        from_owned_fields.push(quote! {
                            #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_owned_variant(data.next()?)?
                        });
                        to_fields.push(quote! {
                            out.push(::perro_api::variant::DeriveVariant::to_variant(&self.#field_ident));
                        });
                        into_fields.push(quote! {
                            out.push(::perro_api::variant::DeriveVariant::into_variant(#field_ident));
                        });
                    }
                }
            }
        }
        Fields::Unnamed(fields) => {
            struct_is_tuple = true;
            for (field_idx, field) in fields.unnamed.into_iter().enumerate() {
                let field_ty = field.ty;
                let field_key = field_idx.to_string();
                let field_key_arc = shared_arc_str(&field_key);
                let tuple_idx = syn::Index::from(field_idx);
                let binding = syn::Ident::new(
                    &format!("__perro_f{field_idx}"),
                    proc_macro2::Span::call_site(),
                );
                tuple_field_idents.push(binding.clone());
                schema_fields.push(field_key.clone());
                codec_hints.push(quote! {
                    __perro_hint_use_derive_variant::<#field_ty>();
                });

                match options.struct_mode {
                    StructMode::Object => {
                        from_fields.push(quote! {
                            <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(obj.get(#field_key)?)?
                        });
                        from_owned_fields.push(quote! {
                            <#field_ty as ::perro_api::variant::DeriveVariant>::from_owned_variant(obj.remove(#field_key)?)?
                        });
                        to_fields.push(quote! {
                            out.insert(#field_key_arc, ::perro_api::variant::DeriveVariant::to_variant(&self.#tuple_idx));
                        });
                        into_fields.push(quote! {
                            out.insert(#field_key_arc, ::perro_api::variant::DeriveVariant::into_variant(#binding));
                        });
                    }
                    StructMode::Array => {
                        from_fields.push(quote! {
                            <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(data.get(#tuple_idx)?)?
                        });
                        from_owned_fields.push(quote! {
                            <#field_ty as ::perro_api::variant::DeriveVariant>::from_owned_variant(data.next()?)?
                        });
                        to_fields.push(quote! {
                            out.push(::perro_api::variant::DeriveVariant::to_variant(&self.#tuple_idx));
                        });
                        into_fields.push(quote! {
                            out.push(::perro_api::variant::DeriveVariant::into_variant(#binding));
                        });
                    }
                }
            }
        }
        Fields::Unit => {
            struct_is_unit = true;
        }
    }

    let from_body = match options.struct_mode {
        StructMode::Object => quote! {
            let obj = value.as_object()?;
            Some(Self( #(#from_fields),* ))
        },
        StructMode::Array => {
            let expected_len = from_fields.len();
            quote! {
                let data = value.as_array()?;
                if data.len() != #expected_len {
                    return None;
                }
                Some(Self( #(#from_fields),* ))
            }
        }
    };
    let from_owned_body = match options.struct_mode {
        StructMode::Object => quote! {
            let mut obj = match value {
                ::perro_api::variant::Variant::Object(obj) => obj,
                _ => return None,
            };
            Some(Self( #(#from_owned_fields),* ))
        },
        StructMode::Array => {
            let expected_len = from_fields.len();
            quote! {
                let data = match value {
                    ::perro_api::variant::Variant::Array(data) => data,
                    _ => return None,
                };
                if data.len() != #expected_len {
                    return None;
                }
                let mut data = data.into_iter();
                Some(Self( #(#from_owned_fields),* ))
            }
        }
    };
    let from_body = if struct_is_unit {
        match options.struct_mode {
            StructMode::Object => quote! {
                if !matches!(value, ::perro_api::variant::Variant::Object(obj) if obj.is_empty()) {
                    return None;
                }
                Some(Self)
            },
            StructMode::Array => quote! {
                if !matches!(value, ::perro_api::variant::Variant::Array(data) if data.is_empty()) {
                    return None;
                }
                Some(Self)
            },
        }
    } else if struct_is_tuple {
        from_body
    } else {
        match options.struct_mode {
            StructMode::Object => quote! {
                let obj = value.as_object()?;
                Some(Self {
                    #(#from_fields,)*
                })
            },
            StructMode::Array => {
                let expected_len = from_fields.len();
                quote! {
                    let data = value.as_array()?;
                    if data.len() != #expected_len {
                        return None;
                    }
                    Some(Self {
                        #(#from_fields,)*
                    })
                }
            }
        }
    };
    let from_owned_body = if struct_is_unit {
        match options.struct_mode {
            StructMode::Object => quote! {
                if !matches!(value, ::perro_api::variant::Variant::Object(obj) if obj.is_empty()) {
                    return None;
                }
                Some(Self)
            },
            StructMode::Array => quote! {
                if !matches!(value, ::perro_api::variant::Variant::Array(data) if data.is_empty()) {
                    return None;
                }
                Some(Self)
            },
        }
    } else if struct_is_tuple {
        from_owned_body
    } else {
        match options.struct_mode {
            StructMode::Object => quote! {
                let mut obj = match value {
                    ::perro_api::variant::Variant::Object(obj) => obj,
                    _ => return None,
                };
                Some(Self {
                    #(#from_owned_fields,)*
                })
            },
            StructMode::Array => {
                let expected_len = from_fields.len();
                quote! {
                    let data = match value {
                        ::perro_api::variant::Variant::Array(data) => data,
                        _ => return None,
                    };
                    if data.len() != #expected_len {
                        return None;
                    }
                    let mut data = data.into_iter();
                    Some(Self {
                        #(#from_owned_fields,)*
                    })
                }
            }
        }
    };

    let field_count = schema_fields.len();
    let to_body = match options.struct_mode {
        StructMode::Object => quote! {
            let mut out = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
            #(#to_fields)*
            ::perro_api::variant::Variant::Object(out)
        },
        StructMode::Array => quote! {
            let mut out = ::std::vec::Vec::<::perro_api::variant::Variant>::with_capacity(#field_count);
            #(#to_fields)*
            ::perro_api::variant::Variant::Array(out)
        },
    };
    let into_body = match options.struct_mode {
        StructMode::Object => quote! {
            let Self { #(#named_field_idents),* } = self;
            let mut out = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
            #(#into_fields)*
            ::perro_api::variant::Variant::Object(out)
        },
        StructMode::Array => quote! {
            let Self { #(#named_field_idents),* } = self;
            let mut out = ::std::vec::Vec::<::perro_api::variant::Variant>::with_capacity(#field_count);
            #(#into_fields)*
            ::perro_api::variant::Variant::Array(out)
        },
    };
    let into_body = if struct_is_unit {
        match options.struct_mode {
            StructMode::Object => quote! {
                let mut out = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                ::perro_api::variant::Variant::Object(out)
            },
            StructMode::Array => quote! {
                let mut out = ::std::vec::Vec::<::perro_api::variant::Variant>::with_capacity(#field_count);
                ::perro_api::variant::Variant::Array(out)
            },
        }
    } else if struct_is_tuple {
        match options.struct_mode {
            StructMode::Object => quote! {
                let Self( #(#tuple_field_idents),* ) = self;
                let mut out = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                #(#into_fields)*
                ::perro_api::variant::Variant::Object(out)
            },
            StructMode::Array => quote! {
                let Self( #(#tuple_field_idents),* ) = self;
                let mut out = ::std::vec::Vec::<::perro_api::variant::Variant>::with_capacity(#field_count);
                #(#into_fields)*
                ::perro_api::variant::Variant::Array(out)
            },
        }
    } else {
        into_body
    };

    let expanded = quote! {
        impl #impl_generics ::perro_api::variant::DeriveVariant for #ident #ty_generics #where_clause {
            fn from_variant(value: &::perro_api::variant::Variant) -> ::core::option::Option<Self> {
                fn __perro_hint_use_derive_variant<T: ::perro_api::variant::DeriveVariant>() {}
                #(#codec_hints)*
                #from_body
            }

            fn from_owned_variant(value: ::perro_api::variant::Variant) -> ::core::option::Option<Self> {
                fn __perro_hint_use_derive_variant<T: ::perro_api::variant::DeriveVariant>() {}
                #(#codec_hints)*
                #from_owned_body
            }

            fn to_variant(&self) -> ::perro_api::variant::Variant {
                #to_body
            }

            fn into_variant(self) -> ::perro_api::variant::Variant {
                #into_body
            }
        }

        impl #impl_generics ::perro_api::variant::VariantSchema for #ident #ty_generics #where_clause {
            fn field_names() -> &'static [&'static str] {
                &[#(#schema_fields),*]
            }
        }

        impl #impl_generics ::core::convert::From<#ident #ty_generics> for ::perro_api::variant::Variant #where_clause {
            fn from(value: #ident #ty_generics) -> Self {
                ::perro_api::variant::DeriveVariant::into_variant(value)
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
    options: VariantDeriveOptions,
) -> TokenStream {
    if options.struct_mode == StructMode::Array {
        // Struct-only option. Keep permissive to avoid errors on shared attrs.
    }
    let mut from_arms = Vec::new();
    let mut from_owned_arms = Vec::new();
    let mut from_unit_string_arms = Vec::new();
    let mut from_owned_unit_string_arms = Vec::new();
    let mut to_arms = Vec::new();
    let mut into_arms = Vec::new();
    let mut codec_hints = Vec::new();
    let mut unit_tag = 0u16;
    let variant_key = shared_arc_str("__variant");
    let data_key = shared_arc_str("__data");
    let mut default_variant = None;

    for variant in variants {
        let variant_ident = variant.ident;
        let variant_name = variant_ident.to_string();
        let variant_name_arc = shared_arc_str(&variant_name);
        let has_default_attr = variant
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("default"));
        if has_default_attr && matches!(variant.fields, Fields::Unit) {
            default_variant = Some(variant_ident.clone());
        }
        match variant.fields {
            Fields::Unit => {
                let numeric_tag = unit_tag;
                unit_tag = unit_tag.wrapping_add(1);
                from_unit_string_arms.push(quote! {
                    #variant_name => Some(Self::#variant_ident),
                });
                from_owned_unit_string_arms.push(quote! {
                    #variant_name => Some(Self::#variant_ident),
                });
                match options.enum_tag_mode {
                    EnumTagMode::String => {
                        from_arms.push(quote! {
                            #variant_name => Some(Self::#variant_ident),
                        });
                        from_owned_arms.push(quote! {
                            #variant_name => Some(Self::#variant_ident),
                        });
                        to_arms.push(quote! {
                            Self::#variant_ident => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::String(#variant_name_arc),
                                );
                            }
                        });
                        into_arms.push(quote! {
                            Self::#variant_ident => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::String(#variant_name_arc),
                                );
                            }
                        });
                    }
                    EnumTagMode::U16 => {
                        from_arms.push(quote! {
                            #numeric_tag => Some(Self::#variant_ident),
                        });
                        from_owned_arms.push(quote! {
                            #numeric_tag => Some(Self::#variant_ident),
                        });
                        to_arms.push(quote! {
                            Self::#variant_ident => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::from(#numeric_tag),
                                );
                            }
                        });
                        into_arms.push(quote! {
                            Self::#variant_ident => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::from(#numeric_tag),
                                );
                            }
                        });
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let mut from_values = Vec::new();
                let mut from_owned_values = Vec::new();
                let mut to_values = Vec::new();
                let mut into_values = Vec::new();
                let mut bindings = Vec::new();
                let expected_len = fields.unnamed.len();

                for (idx, field) in fields.unnamed.into_iter().enumerate() {
                    let field_ty = field.ty;
                    codec_hints.push(quote! {
                        __perro_hint_use_derive_variant::<#field_ty>();
                    });
                    let binding = syn::Ident::new(
                        &format!("__perro_v{}", idx),
                        proc_macro2::Span::call_site(),
                    );
                    let index = syn::Index::from(idx);
                    from_values.push(quote! {
                        <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(data.get(#index)?)?
                    });
                    from_owned_values.push(quote! {
                        <#field_ty as ::perro_api::variant::DeriveVariant>::from_owned_variant(data.next()?)?
                    });
                    to_values.push(quote! {
                        ::perro_api::variant::DeriveVariant::to_variant(#binding)
                    });
                    into_values.push(quote! {
                        ::perro_api::variant::DeriveVariant::into_variant(#binding)
                    });
                    bindings.push(binding);
                }

                let numeric_tag = unit_tag;
                unit_tag = unit_tag.wrapping_add(1);
                match options.enum_tag_mode {
                    EnumTagMode::String => {
                        to_arms.push(quote! {
                            Self::#variant_ident( #( #bindings ),* ) => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::String(#variant_name_arc),
                                );
                                let data = vec![#(#to_values),*];
                                out.insert(
                                    #data_key,
                                    ::perro_api::variant::Variant::Array(data),
                                );
                            }
                        });
                        into_arms.push(quote! {
                            Self::#variant_ident( #( #bindings ),* ) => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::String(#variant_name_arc),
                                );
                                let data = vec![#(#into_values),*];
                                out.insert(
                                    #data_key,
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
                        from_owned_arms.push(quote! {
                            #variant_name => {
                                let data = match obj.remove("__data")? {
                                    ::perro_api::variant::Variant::Array(data) => data,
                                    _ => return None,
                                };
                                if data.len() != #expected_len {
                                    return None;
                                }
                                let mut data = data.into_iter();
                                Some(Self::#variant_ident( #(#from_owned_values),* ))
                            }
                        });
                    }
                    EnumTagMode::U16 => {
                        to_arms.push(quote! {
                            Self::#variant_ident( #( #bindings ),* ) => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::from(#numeric_tag),
                                );
                                let data = vec![#(#to_values),*];
                                out.insert(
                                    #data_key,
                                    ::perro_api::variant::Variant::Array(data),
                                );
                            }
                        });
                        into_arms.push(quote! {
                            Self::#variant_ident( #( #bindings ),* ) => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::from(#numeric_tag),
                                );
                                let data = vec![#(#into_values),*];
                                out.insert(
                                    #data_key,
                                    ::perro_api::variant::Variant::Array(data),
                                );
                            }
                        });
                        from_arms.push(quote! {
                            #numeric_tag => {
                                let data = obj.get("__data")?.as_array()?;
                                if data.len() != #expected_len {
                                    return None;
                                }
                                Some(Self::#variant_ident( #(#from_values),* ))
                            }
                        });
                        from_owned_arms.push(quote! {
                            #numeric_tag => {
                                let data = match obj.remove("__data")? {
                                    ::perro_api::variant::Variant::Array(data) => data,
                                    _ => return None,
                                };
                                if data.len() != #expected_len {
                                    return None;
                                }
                                let mut data = data.into_iter();
                                Some(Self::#variant_ident( #(#from_owned_values),* ))
                            }
                        });
                    }
                }
            }
            Fields::Named(fields) => {
                let mut from_fields = Vec::new();
                let mut from_owned_fields = Vec::new();
                let mut to_fields = Vec::new();
                let mut into_fields = Vec::new();
                let mut bindings = Vec::new();

                for field in fields.named {
                    let Some(field_ident) = field.ident else {
                        continue;
                    };
                    let field_ty = field.ty;
                    codec_hints.push(quote! {
                        __perro_hint_use_derive_variant::<#field_ty>();
                    });
                    let key = field_ident.to_string();
                    let key_arc = shared_arc_str(&key);
                    from_fields.push(quote! {
                        #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_variant(data.get(#key)?)?
                    });
                    from_owned_fields.push(quote! {
                        #field_ident: <#field_ty as ::perro_api::variant::DeriveVariant>::from_owned_variant(data.remove(#key)?)?
                    });
                    to_fields.push(quote! {
                        data.insert(
                            #key_arc,
                            ::perro_api::variant::DeriveVariant::to_variant(#field_ident),
                        );
                    });
                    into_fields.push(quote! {
                        data.insert(
                            #key_arc,
                            ::perro_api::variant::DeriveVariant::into_variant(#field_ident),
                        );
                    });
                    bindings.push(field_ident);
                }

                let numeric_tag = unit_tag;
                unit_tag = unit_tag.wrapping_add(1);
                match options.enum_tag_mode {
                    EnumTagMode::String => {
                        to_arms.push(quote! {
                            Self::#variant_ident { #( #bindings ),* } => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::String(#variant_name_arc),
                                );
                                let mut data = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                                #(#to_fields)*
                                out.insert(
                                    #data_key,
                                    ::perro_api::variant::Variant::Object(data),
                                );
                            }
                        });
                        into_arms.push(quote! {
                            Self::#variant_ident { #( #bindings ),* } => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::String(#variant_name_arc),
                                );
                                let mut data = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                                #(#into_fields)*
                                out.insert(
                                    #data_key,
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
                        from_owned_arms.push(quote! {
                            #variant_name => {
                                let mut data = match obj.remove("__data")? {
                                    ::perro_api::variant::Variant::Object(data) => data,
                                    _ => return None,
                                };
                                Some(Self::#variant_ident {
                                    #(#from_owned_fields),*
                                })
                            }
                        });
                    }
                    EnumTagMode::U16 => {
                        to_arms.push(quote! {
                            Self::#variant_ident { #( #bindings ),* } => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::from(#numeric_tag),
                                );
                                let mut data = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                                #(#to_fields)*
                                out.insert(
                                    #data_key,
                                    ::perro_api::variant::Variant::Object(data),
                                );
                            }
                        });
                        into_arms.push(quote! {
                            Self::#variant_ident { #( #bindings ),* } => {
                                out.insert(
                                    #variant_key,
                                    ::perro_api::variant::Variant::from(#numeric_tag),
                                );
                                let mut data = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                                #(#into_fields)*
                                out.insert(
                                    #data_key,
                                    ::perro_api::variant::Variant::Object(data),
                                );
                            }
                        });
                        from_arms.push(quote! {
                            #numeric_tag => {
                                let data = obj.get("__data")?.as_object()?;
                                Some(Self::#variant_ident {
                                    #(#from_fields),*
                                })
                            }
                        });
                        from_owned_arms.push(quote! {
                            #numeric_tag => {
                                let mut data = match obj.remove("__data")? {
                                    ::perro_api::variant::Variant::Object(data) => data,
                                    _ => return None,
                                };
                                Some(Self::#variant_ident {
                                    #(#from_owned_fields),*
                                })
                            }
                        });
                    }
                }
            }
        }
    }

    let null_default_ref = default_variant.as_ref().map(|variant_ident| {
        quote! {
            if matches!(value, ::perro_api::variant::Variant::Null) {
                return Some(Self::#variant_ident);
            }
        }
    });
    let null_default_owned = default_variant.as_ref().map(|variant_ident| {
        quote! {
            if matches!(value, ::perro_api::variant::Variant::Null) {
                return Some(Self::#variant_ident);
            }
        }
    });

    let tag_read = match options.enum_tag_mode {
        EnumTagMode::String => quote! {
            let tag = obj.get("__variant")?.as_str()?;
        },
        EnumTagMode::U16 => quote! {
            let tag = obj.get("__variant")?.as_u16()?;
        },
    };
    let tag_owned_read = match options.enum_tag_mode {
        EnumTagMode::String => quote! {
            let tag = match obj.remove("__variant")? {
                ::perro_api::variant::Variant::String(tag) => tag,
                _ => return None,
            };
            let tag = tag.as_ref();
        },
        EnumTagMode::U16 => quote! {
            let tag_value = obj.remove("__variant")?;
            let tag = tag_value.as_u16()?;
        },
    };

    let expanded = quote! {
        impl #impl_generics ::perro_api::variant::DeriveVariant for #ident #ty_generics #where_clause {
            fn from_variant(value: &::perro_api::variant::Variant) -> ::core::option::Option<Self> {
                fn __perro_hint_use_derive_variant<T: ::perro_api::variant::DeriveVariant>() {}
                #(#codec_hints)*
                #null_default_ref
                if let Some(tag) = value.as_str() {
                    return match tag {
                        #(#from_unit_string_arms)*
                        _ => None,
                    };
                }
                let obj = value.as_object()?;
                #tag_read
                match tag {
                    #(#from_arms)*
                    _ => None,
                }
            }

            fn from_owned_variant(value: ::perro_api::variant::Variant) -> ::core::option::Option<Self> {
                fn __perro_hint_use_derive_variant<T: ::perro_api::variant::DeriveVariant>() {}
                #(#codec_hints)*
                #null_default_owned
                if let ::perro_api::variant::Variant::String(tag) = &value {
                    return match tag.as_ref() {
                        #(#from_owned_unit_string_arms)*
                        _ => None,
                    };
                }
                let mut obj = match value {
                    ::perro_api::variant::Variant::Object(obj) => obj,
                    _ => return None,
                };
                #tag_owned_read
                match tag {
                    #(#from_owned_arms)*
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

            fn into_variant(self) -> ::perro_api::variant::Variant {
                let mut out = ::std::collections::BTreeMap::<::std::sync::Arc<str>, ::perro_api::variant::Variant>::new();
                match self {
                    #(#into_arms),*
                }
                ::perro_api::variant::Variant::Object(out)
            }
        }

        impl #impl_generics ::perro_api::variant::VariantSchema for #ident #ty_generics #where_clause {}

        impl #impl_generics ::core::convert::From<#ident #ty_generics> for ::perro_api::variant::Variant #where_clause {
            fn from(value: #ident #ty_generics) -> Self {
                ::perro_api::variant::DeriveVariant::into_variant(value)
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
        if attr.path().is_ident("expose") || attr.path().is_ident("node_ref") {
            continue;
        }
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

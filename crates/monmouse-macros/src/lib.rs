use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(InputFields, attributes(validate))]
pub fn derive_input_fields(input: TokenStream) -> TokenStream {
    expand_input_fields(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_input_fields(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "InputFields requires a struct",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "InputFields requires named fields",
        ));
    };
    let mut methods = Vec::new();
    for field in &fields.named {
        let ident = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        let binding = format_ident!("{}_field", ident);
        let cfg: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"))
            .collect();
        let rules: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("validate"))
            .collect();
        let validator = if rules.is_empty() {
            quote! { |_| Ok(()) }
        } else {
            quote! {
                |value| {
                    // A single-field helper cannot execute the owning struct's sibling rules.
                    #[derive(::validator::Validate)]
                    struct FieldValidation<'a> {
                        #(#rules)*
                        #ident: &'a #ty,
                    }
                    ::validator::Validate::validate(&FieldValidation { #ident: value })
                }
            }
        };
        methods.push(quote! {
            #(#cfg)*
            pub fn #binding(&mut self) -> crate::setting::validation::ValidatedField<'_, #ty> {
                crate::setting::validation::ValidatedField {
                    id: concat!(module_path!(), "::", stringify!(#name), ".", stringify!(#ident)),
                    value: &mut self.#ident,
                    validator: #validator,
                }
            }
        });
    }
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #(#methods)*
        }
    })
}

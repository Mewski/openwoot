use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DataEnum, DeriveInput, Fields, Result};

use crate::input;

pub(crate) fn derive(input: &DeriveInput) -> Result<TokenStream> {
    input::check(input)?;
    let wire = input::wire();
    let generics = input::bounded(&input.generics, &quote!(#wire::Encode));
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let name = &input.ident;
    let body = match &input.data {
        Data::Struct(data) => fields_of_self(&data.fields, &wire),
        Data::Enum(data) => {
            input::variant_count(input, data.variants.len())?;
            variants(data, &wire)
        }
        Data::Union(_) => TokenStream::new(),
    };
    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #wire::Encode for #name #type_generics #where_clause {
            #[inline]
            fn encode(&self, writer: &mut #wire::Writer<'_>) {
                #body
            }
        }
    })
}

fn fields_of_self(fields: &Fields, wire: &TokenStream) -> TokenStream {
    if fields.is_empty() {
        return quote!(let _ = writer;);
    }
    let encoded = fields
        .members()
        .map(|member| quote!(#wire::Encode::encode(&self.#member, writer);));
    quote!(#(#encoded)*)
}

fn variants(data: &DataEnum, wire: &TokenStream) -> TokenStream {
    let indices = data.variants.iter().zip(0_u32..).map(|(variant, index)| {
        let ident = &variant.ident;
        match &variant.fields {
            Fields::Unit => quote!(Self::#ident => #index,),
            Fields::Unnamed(_) => quote!(Self::#ident(..) => #index,),
            Fields::Named(_) => quote!(Self::#ident { .. } => #index,),
        }
    });
    let variant = quote!(writer.variant(match self { #(#indices)* }););
    if data
        .variants
        .iter()
        .all(|variant| variant.fields.is_empty())
    {
        return variant;
    }
    let payloads = data.variants.iter().map(|variant| {
        let ident = &variant.ident;
        let bindings: Vec<_> = (0..variant.fields.len())
            .map(|position| format_ident!("field_{position}"))
            .collect();
        let pattern = match &variant.fields {
            Fields::Unit => quote!(Self::#ident),
            Fields::Unnamed(_) => quote!(Self::#ident(#(#bindings),*)),
            Fields::Named(_) => {
                let members = variant.fields.members();
                quote!(Self::#ident { #(#members: #bindings),* })
            }
        };
        let encoded = bindings
            .iter()
            .map(|binding| quote!(#wire::Encode::encode(#binding, writer);));
        quote!(#pattern => { #(#encoded)* })
    });
    quote! {
        #variant
        match self { #(#payloads)* }
    }
}

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataEnum, DeriveInput, Fields, Result};

use crate::input;

pub(crate) fn derive(input: &DeriveInput) -> Result<TokenStream> {
    input::check(input)?;
    let wire = input::wire();
    let generics = input::bounded(&input.generics, &quote!(#wire::Decode));
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let name = &input.ident;
    let body = match &input.data {
        Data::Struct(data) if data.fields.is_empty() => {
            let value = construct(&quote!(Self), &data.fields, &[]);
            quote!(let _ = reader; #value)
        }
        Data::Struct(data) => construct(&quote!(Self), &data.fields, &decoded(&data.fields, &wire)),
        Data::Enum(data) => variants(
            data,
            input::variant_count(input, data.variants.len())?,
            &wire,
        ),
        Data::Union(_) => TokenStream::new(),
    };
    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #wire::Decode for #name #type_generics #where_clause {
            #[inline]
            fn decode(reader: &mut #wire::Reader<'_>) -> Self {
                #body
            }
        }
    })
}

fn variants(data: &DataEnum, count: u32, wire: &TokenStream) -> TokenStream {
    let values: Vec<_> = data
        .variants
        .iter()
        .map(|variant| {
            let ident = &variant.ident;
            let values = decoded(&variant.fields, wire);
            construct(&quote!(Self::#ident), &variant.fields, &values)
        })
        .collect();
    match values.split_last() {
        Some((last, [])) => quote!(let _ = reader.variant(#count); #last),
        Some((last, others)) => {
            let arms = others
                .iter()
                .zip(0_u32..)
                .map(|(value, index)| quote!(#index => #value,));
            quote!(match reader.variant(#count) { #(#arms)* _ => #last, })
        }
        None => TokenStream::new(),
    }
}

fn decoded(fields: &Fields, wire: &TokenStream) -> Vec<TokenStream> {
    fields
        .iter()
        .map(|_| quote!(#wire::Decode::decode(reader)))
        .collect()
}

fn construct(path: &TokenStream, fields: &Fields, values: &[TokenStream]) -> TokenStream {
    match fields {
        Fields::Unit => quote!(#path),
        Fields::Unnamed(_) => quote!(#path(#(#values),*)),
        Fields::Named(named) => {
            let idents = named.named.iter().filter_map(|field| field.ident.as_ref());
            quote!(#path { #(#idents: #values),* })
        }
    }
}

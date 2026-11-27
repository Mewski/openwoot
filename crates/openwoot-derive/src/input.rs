use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, Generics, Result, parse_quote};

pub(crate) fn wire() -> TokenStream {
    quote!(::openwoot_protocol::wire)
}

pub(crate) fn check(input: &DeriveInput) -> Result<()> {
    let mut attributes: Vec<&Attribute> = input.attrs.iter().collect();
    match &input.data {
        Data::Struct(data) => {
            attributes.extend(data.fields.iter().flat_map(|field| &field.attrs));
        }
        Data::Enum(data) if data.variants.is_empty() => {
            return Err(Error::new_spanned(
                &input.ident,
                "an enum without variants has no wire format",
            ));
        }
        Data::Enum(data) => {
            for variant in &data.variants {
                attributes.extend(&variant.attrs);
                attributes.extend(variant.fields.iter().flat_map(|field| &field.attrs));
            }
        }
        Data::Union(_) => {
            return Err(Error::new_spanned(
                &input.ident,
                "a union has no wire format",
            ));
        }
    }
    attributes
        .into_iter()
        .find(|attribute| attribute.path().is_ident("serde"))
        .map_or(Ok(()), |attribute| {
            Err(Error::new_spanned(
                attribute,
                "`serde` attributes would make postcard and the wire codec disagree",
            ))
        })
}

pub(crate) fn bounded(generics: &Generics, bound: &TokenStream) -> Generics {
    let mut generics = generics.clone();
    let parameters: Vec<_> = generics
        .type_params()
        .map(|parameter| parameter.ident.clone())
        .collect();
    let clause = generics.make_where_clause();
    for parameter in parameters {
        clause.predicates.push(parse_quote!(#parameter: #bound));
    }
    generics
}

pub(crate) fn variant_count(input: &DeriveInput, variants: usize) -> Result<u32> {
    u32::try_from(variants)
        .map_err(|_| Error::new_spanned(&input.ident, "the enum has more variants than a `u32`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_attributes_unions_and_empty_enums_have_no_wire_format() {
        let renamed: DeriveInput = parse_quote! { struct Renamed { #[serde(rename = "b")] a: u8 } };
        let skipped: DeriveInput = parse_quote! { enum Skipped { #[serde(skip)] A, B } };
        let union: DeriveInput = parse_quote! { union Union { a: u8 } };
        let empty: DeriveInput = parse_quote! { enum Empty {} };
        for input in [renamed, skipped, union, empty] {
            assert!(check(&input).is_err(), "{}", input.ident);
        }
        let plain: DeriveInput = parse_quote! { enum Plain { A, B(u8), C { d: u16 } } };
        assert!(check(&plain).is_ok());
    }

    #[test]
    fn every_type_parameter_takes_the_bound() {
        let input: DeriveInput = parse_quote! { struct Change<T, U> { a: T, b: U } };
        let generics = bounded(&input.generics, &quote!(Encode));
        let predicates = generics.where_clause.map(|clause| clause.predicates.len());
        assert_eq!(predicates, Some(2));
    }
}

mod decode;
mod encode;
mod input;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Encode)]
pub fn derive_encode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    encode::derive(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Decode)]
pub fn derive_decode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    decode::derive(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn node(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let vis = &input.vis;
    let block = &input.block;

    let output = quote! {
        #vis fn #name(inputs: std::collections::HashMap<String, serde_json::Value>) -> std::collections::HashMap<String, serde_json::Value> {
            #block
        }
    };

    output.into()
}
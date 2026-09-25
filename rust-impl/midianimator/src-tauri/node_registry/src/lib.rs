use proc_macro::TokenStream;

/// marks a function as a node executor, build.rs finds these and adds them to the node registry.
/// the function itself is left as is, it has to be `fn(Inputs) -> NodeResult`
#[proc_macro_attribute]
pub fn node(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

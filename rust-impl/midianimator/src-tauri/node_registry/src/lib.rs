use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, GenericArgument, ItemFn, Pat, PathArguments, Type};

/// marks a function as a node executor, build.rs finds these and adds them to the node registry.
///
/// the function is written with typed arguments named after its input ids, the macro turns it into a
/// `fn(&Inputs) -> NodeResult` of the same name that pulls each argument out of the inputs:
///
/// - `note: &MIDINote` is a required input, converted to that type if the value is something else
/// - `name: Option<&String>` is an optional input, `None` when it's missing or null
/// - `count: f64` (not a reference) is a required input, cloned out
/// - `inputs: &Inputs` gets all the inputs, for dynamic inputs (`Inputs::dynamic`)
///
/// a leading underscore is left off the input id (`_inputs`)
#[proc_macro_attribute]
pub fn node(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let attrs = &func.attrs;
    let vis = &func.vis;
    let name = &func.sig.ident;

    // the typed function, nested inside the wrapper so its name doesn't clash
    let mut typed = func.clone();
    typed.attrs.clear();
    typed.vis = syn::Visibility::Inherited;
    typed.sig.ident = format_ident!("__typed");

    let mut lets = Vec::new();
    let mut args = Vec::new();
    for (i, arg) in func.sig.inputs.iter().enumerate() {
        let FnArg::Typed(arg) = arg else {
            return syn::Error::new_spanned(arg, "node executors can't take self").to_compile_error().into();
        };
        let Pat::Ident(ident) = &*arg.pat else {
            return syn::Error::new_spanned(&arg.pat, "node executor arguments must be plain names").to_compile_error().into();
        };
        let key = ident.ident.to_string().trim_start_matches('_').to_string();
        let var = format_ident!("__input_{}", i);

        match &*arg.ty {
            // all the inputs
            Type::Reference(reference) if is_named(&reference.elem, "Inputs") => args.push(quote!(inputs)),
            // required, borrowed
            Type::Reference(reference) => {
                let ty = &reference.elem;
                lets.push(quote!(let #var = inputs.value::<#ty>(#key)?;));
                args.push(quote!(#var.get::<#ty>()));
            }
            ty => match option_inner(ty) {
                // optional, borrowed
                Some(Type::Reference(reference)) => {
                    let ty = &reference.elem;
                    lets.push(quote!(let #var = inputs.value_opt::<#ty>(#key)?;));
                    args.push(quote!(#var.as_ref().map(|v| v.get::<#ty>())));
                }
                // optional, cloned out
                Some(inner) => {
                    lets.push(quote!(let #var = inputs.value_opt::<#inner>(#key)?;));
                    args.push(quote!(#var.as_ref().map(|v| v.get::<#inner>().clone())));
                }
                // required, cloned out
                None => {
                    lets.push(quote!(let #var = inputs.value::<#ty>(#key)?;));
                    args.push(quote!(#var.get::<#ty>().clone()));
                }
            },
        }
    }

    quote! {
        #(#attrs)*
        #vis fn #name(inputs: &crate::graph::executors::io::Inputs) -> crate::graph::executors::io::NodeResult {
            #typed
            #(#lets)*
            __typed(#(#args),*)
        }
    }
    .into()
}

/// true if the type's last path segment is `name`
fn is_named(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|s| s.ident == name))
}

/// `T` for `Option<T>`
fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.first()? {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics, Index,
};

#[proc_macro_derive(CopyFrom)]
pub fn copy_from_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    // Add a bound `T: HeapSize` to every type parameter T.
    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let copy_from_lines = copy_from_lines(&input.data);

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        // The generated impl.
        impl #impl_generics copy_from::CopyFrom for #name #ty_generics #where_clause {
            fn copy_from(&mut self, other: &Self) {
                #copy_from_lines
            }
        }
    };

    // Hand the output tokens back to the compiler
    proc_macro::TokenStream::from(expanded)
}

// Add a bound `T: HeapSize` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(copy_from::CopyFrom));
        }
    }
    generics
}

fn copy_from_lines(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    let recurse = fields.named.iter().map(|f| {
                        let name = &f.ident;
                        quote_spanned! {f.span()=>
                            copy_from::CopyFrom::copy_from(&mut self.#name, &other.#name);
                        }
                    });
                    quote! {
                        #(#recurse)*;
                    }
                }
                Fields::Unnamed(ref fields) => {
                    let recurse = fields.unnamed.iter().enumerate().map(|(i, f)| {
                        let index = Index::from(i);
                        quote_spanned! {f.span()=>
                            copy_from::CopyFrom::copy_from(&mut self.#index, &other.#index);
                        }
                    });
                    quote! {
                        #(#recurse)*;
                    }
                }
                Fields::Unit => {
                    // Unit structs cannot copy
                    quote!()
                }
            }
        }
        // Data::Enum(ref data)  => {
        //     data.variants.iter().map { |v|j
        //     }
        // }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}

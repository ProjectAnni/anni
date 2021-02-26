use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(FromFile)]
pub fn derive_from_file(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    let name = &ast.ident;
    let gen = quote! {
        impl FromFile for #name {
            fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
                Ok(Self::from_str(&*std::fs::read_to_string(path.as_ref())?)?)
            }
        }
    };
    gen.into()
}

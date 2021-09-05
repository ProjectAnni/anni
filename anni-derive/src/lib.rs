use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input, Data, DataStruct, Fields, DataEnum};

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

#[proc_macro_derive(ClapHandler, attributes(clap_handler))]
pub fn derive_clap_handler(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let attrs = &input.attrs;
    let argument_type = attrs.iter().find_map(|attr| {
        if attr.path.is_ident("clap_handler") {
            let attr: syn::Ident = attrs[0].parse_args().expect("Failed to get argument type");
            Some(attr)
        } else {
            None
        }
    });

    let expanded = match input.data {
        Data::Struct(DataStruct { ref fields, .. }) => {
            match fields {
                Fields::Named(ref fields_name) => {
                    // find struct field which has #[clap(subcommand)]
                    let subcommand_field = fields_name.named.iter().find_map(|field| {
                        for attr in field.attrs.iter() {
                            if attr.path.is_ident("clap") {
                                let ident: syn::Ident = attr.parse_args().ok()?;
                                if ident == "subcommand" {
                                    return Some(field.ident.clone().unwrap());
                                }
                            }
                        }
                        None
                    }).expect("No subcommand found!");
                    match argument_type {
                        Some(argument_type) => {
                            quote! {
                                impl anni_common::traits::HandleArgs<#argument_type> for #name {
                                    #[inline(always)]
                                    fn handle(&self, arg: &#argument_type) -> anyhow::Result<()> {
                                        self.#subcommand_field.handle(arg)
                                    }
                                }
                            }
                        }
                        None => {
                            quote! {
                                impl anni_common::traits::Handle for #name {
                                    #[inline(always)]
                                    fn handle_subcommand(&self) -> anyhow::Result<()> {
                                        self.#subcommand_field.handle()
                                    }
                                }
                            }
                        }
                    }
                }
                _ => panic!("ClapHandler is not implemented for unnamed or None struct"),
            }
        }
        Data::Enum(DataEnum { variants, .. }) => {
            // list enum variants
            let subcommands: Vec<_> = variants.iter().map(|v| {
                let ident = &v.ident;
                quote! { #name::#ident }
            }).collect();
            match argument_type {
                Some(argument_type) => {
                    quote! {
                       impl anni_common::traits::HandleArgs<#argument_type> for #name {
                            #[inline(always)]
                            fn handle(&self, arg: &#argument_type) -> anyhow::Result<()> {
                                match self {
                                    #(#subcommands(s) => s.handle(arg),)*
                                }
                            }
                        }
                    }
                }
                None => {
                    quote! {
                        impl anni_common::traits::Handle for #name {
                            #[inline(always)]
                            fn handle_subcommand(&self) -> anyhow::Result<()> {
                                match self {
                                    #(#subcommands(s) => s.handle(),)*
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => panic!("ClapHandler is not implemented for union type"),
    };
    expanded.into()
}

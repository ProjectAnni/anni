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

#[proc_macro_derive(Handler, attributes(clap_handler, clap_handler_arg))]
pub fn derive_clap_handler(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let attrs = &input.attrs;
    let handler_func = attrs.iter().find_map(|attr| {
        if attr.path.is_ident("clap_handler") {
            let attr: syn::Ident = attr.parse_args().expect("Failed to get handler function");
            Some(attr)
        } else {
            None
        }
    });
    let argument_type = attrs.iter().find_map(|attr| {
        if attr.path.is_ident("clap_handler_arg") {
            let attr: syn::Ident = attr.parse_args().expect("Failed to get argument type");
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
                    let subcommand_field: Option<syn::Ident> = fields_name.named.iter().find_map(|field| {
                        for attr in field.attrs.iter() {
                            if attr.path.is_ident("clap") {
                                let ident: syn::Ident = attr.parse_args().ok()?;
                                if ident == "subcommand" {
                                    return Some(field.ident.clone().unwrap());
                                }
                            }
                        }
                        None
                    });

                    // region
                    // subcommand, handler, argument
                    // if exists: [subcommand]
                    //   As subcommand exists, the struct would not be a leaf subcommand(action).
                    //   But no argument is provided, so follow the default behavior, generate handle_command for struct.
                    //   ```
                    //   impl Handle for #name {
                    //       fn handle_subcommand(&self) -> anyhow::Result<()> {
                    //           self.#subcommand.handle();
                    //       }
                    //   }
                    //   ```
                    //
                    // if exists: [subcommand, argument]
                    //   No handler detected, so no one can provide the required argument.
                    //   INVALID form.
                    //
                    // if exists: [subcommand, handler]
                    //   The handler should be executed before handle_subcommand.
                    //   Override both handle and handle_subcommand methods.
                    //   ```
                    //   impl Handle for #name {
                    //       fn handle(&self) -> anyhow::Result<()> {
                    //           #handler(self)?;
                    //           self.handle_subcommand()
                    //       }
                    //   }
                    //   ```
                    //
                    // if exists: [subcommand, handler, argument]
                    //   The return value of handler should be passed to subcommand as argument.
                    //   ```
                    //   impl Handle for #name {
                    //       fn handle(&self) -> anyhow::Result<()> {
                    //           let arg: #argument = #handler(self)?;
                    //           self.#subcommand.handle(&arg)
                    //       }
                    //   }
                    //   ```
                    //
                    // if exists: [handler]
                    //   No subcommand found. It is a leaf subcommand(action).
                    //   ```
                    //   impl Handle for #name {
                    //       fn handle(&self) -> anyhow::Result<()> {
                    //           #handler()
                    //       }
                    //       // handle_subcommand is useless
                    //   }
                    //   ```
                    //
                    // if exists: [handler, argument]
                    //   Handle #argument.
                    //   ```
                    //   impl HandleArgs<#argument> for #name {
                    //       fn handle(&self, arg: &#argument) {
                    //           #handler(self, arg)
                    //       }
                    //   }
                    //
                    // if exists: [argument]
                    //   Only argument is provided.
                    //   INVALID form.
                    // endregion
                    match subcommand_field {
                        Some(subcommand_field) => {
                            if argument_type.is_some() && handler_func.is_none() {
                                panic!("Invalid format!");
                            }

                            let handle_subcommand_impl = match argument_type.is_some() {
                                // Argument provided
                                // Process should be completed in `handle`
                                true => quote! {},
                                // No argument provided
                                // impl default `handle_subcommand`
                                false => quote! {
                                    #[inline(always)]
                                    fn handle_subcommand(&self) -> anyhow::Result<()> {
                                        use anni_common::traits::Handle;
                                        self.#subcommand_field.handle()
                                    }
                                }
                            };
                            let handle_impl = match handler_func {
                                // Handler detected
                                Some(handler_func) => {
                                    let body = match argument_type {
                                        // Argument exists, pass it to `subcommand.handle`
                                        Some(argument_type) => quote! {
                                            use anni_common::traits::HandleArgs;
                                            let arg: #argument_type = #handler_func(self)?;
                                            self.#subcommand_field.handle(&arg)
                                        },
                                        // No argument provided
                                        None => quote! {
                                            use anni_common::traits::Handle;
                                            #handler_func(self)?;
                                            self.#subcommand_field.handle()
                                        },
                                    };
                                    quote! {
                                        fn handle(&self) -> anyhow::Result<()> {
                                            #body
                                        }
                                    }
                                }
                                // No handler, `handle` should not be override
                                None => quote! {},
                            };
                            quote! {
                                impl anni_common::traits::Handle for #name {
                                    #handle_impl
                                    #handle_subcommand_impl
                                }
                            }
                        }
                        None => {
                            // no subcommand field, leaf action
                            // handler must exist
                            match handler_func {
                                Some(handler_func) => {
                                    // trait to impl
                                    let impl_trait = match argument_type.clone() {
                                        Some(argument_type) => quote! { anni_common::traits::HandleArgs<#argument_type> },
                                        None => quote! { anni_common::traits::Handle },
                                    };
                                    // function signature
                                    let func_sign = match argument_type.clone() {
                                        Some(argument_type) => quote! { &self, arg: &#argument_type },
                                        None => quote! { &self },
                                    };
                                    // handler call
                                    let handler_call = match argument_type {
                                        Some(_) => quote! {
                                            use anni_common::traits::HandleArgs;
                                            #handler_func(self, arg)
                                        },
                                        None => quote! {
                                            use anni_common::traits::Handle;
                                            #handler_func(self)
                                        },
                                    };
                                    quote! {
                                        impl #impl_trait for #name {
                                            fn handle(#func_sign) -> anyhow::Result<()> {
                                                #handler_call
                                            }
                                        }
                                    }
                                }
                                None => panic!("clap_handler function must exist!"),
                            }
                        }
                    }
                }
                _ => panic!("Handler is not implemented for unnamed or None struct"),
            }
        }
        Data::Enum(DataEnum { variants, .. }) => {
            // panic if handler_func exists
            if handler_func.is_some() {
                panic!("clap_handler is not available on enums");
            }

            // list enum variants
            let subcommands: Vec<_> = variants.iter().map(|v| {
                let ident = &v.ident;
                quote! { #name::#ident }
            }).collect();

            let impl_trait = match argument_type.clone() {
                Some(argument_type) => quote! { anni_common::traits::HandleArgs<#argument_type> },
                None => quote! { anni_common::traits::Handle },
            };

            let handle_signature = match argument_type.clone() {
                Some(argument_type) => quote! { handle(&self, arg: &#argument_type) -> anyhow::Result<()> },
                None => quote! { handle_subcommand(&self) -> anyhow::Result<()> },
            };

            let handle_call = match argument_type {
                Some(_) => quote! { s.handle(arg) },
                None => quote! { s.handle() },
            };

            quote! {
                impl #impl_trait for #name {
                    #[inline(always)]
                    fn #handle_signature {
                        match self {
                            #(#subcommands(s) => #handle_call,)*
                        }
                    }
                }
            }
        }
        _ => panic!("Handler is not implemented for union type"),
    };
    expanded.into()
}

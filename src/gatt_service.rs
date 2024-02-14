use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{Data, DataStruct, Expr, ExprLit, Fields, FieldsNamed, Token};

use super::util::{get_valid_arg, ArgValueType};

pub struct GattServiceArgs {
    pub uuid: Option<ExprLit>,
    pub path: Option<ExprLit>,
    pub primary: Option<ExprLit>,
}

impl GattServiceArgs {
    fn new(value_map: HashMap<String, ArgValueType>, stream: ParseStream) -> Result<Self> {
        let mut uuid = None;
        let mut primary = None;
        let mut path = None;
        if let ArgValueType::Str(Some(val)) = value_map.get("uuid").unwrap() {
            uuid = Some(val.to_owned());
        }
        if let ArgValueType::Bool(val) = value_map.get("primary").unwrap() {
            primary = val.to_owned();
        }
        if let ArgValueType::Str(Some(val)) = value_map.get("path").unwrap() {
            path = Some(val.to_owned());
        }

        if let None = uuid {
            return Err(stream.error("uuid must be defined"));
        }

        Ok(Self {
            uuid,
            primary,
            path,
        })
    }
}

impl Parse for GattServiceArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let expressions = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;

        let mut arg_map: HashMap<String, ArgValueType> = HashMap::from([
            ("uuid".into(), ArgValueType::Str(None)),
            ("primary".into(), ArgValueType::Bool(None)),
            ("path".into(), ArgValueType::Str(None)),
        ]);

        for expr in &expressions {
            let (arg_name, arg_value) = get_valid_arg(&arg_map, expr, &input)?;
            arg_map.insert(arg_name, arg_value);
        }

        GattServiceArgs::new(arg_map, input)
    }
}

pub fn apply_macro(
    ast: &syn::DeriveInput,
    uuid: ExprLit,
    primary: Option<ExprLit>,
    path: Option<ExprLit>,
) -> TokenStream {
    let name = &ast.ident;
    let name_str = name.clone().to_string();

    let visibility = &ast.vis;
    // todo!
    let _generics = &ast.generics;

    let mut extra_props = quote! {};
    if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = &ast.data
    {
        extra_props = quote! { #named };
    }

    let service_path = match path {
        Some(expr) => quote! { #expr },
        None => quote! { "" },
    };

    let primary = match primary {
        Some(expr) => quote! { #expr },
        None => quote! { true },
    };

    let gen = quote! {
        use gattrs::*;
        use gattrs::zbus::*;

        #[derive(derivative::Derivative)]
        #[derivative(Default)]
        #visibility struct #name {
            characteristic_paths: Vec<zbus::zvariant::ObjectPath<'static>>,

            #extra_props
        }

        #[dbus_interface(name = "org.bluez.GattService1")]
        impl #name {
            #[dbus_interface(property, name = "UUID")]
            fn uuid(&self) -> &str {
                #uuid
            }

            #[dbus_interface(property)]
            fn primary(&self) -> bool {
                #primary
            }

            #[dbus_interface(property)]
            fn characteristics(&self) -> Vec<zbus::zvariant::ObjectPath<'static>> {
                 self.characteristic_paths.clone()
            }
         }

         #[gattrs::async_trait::async_trait]
         impl gattrs::gatt::ServiceRegister for #name {
            fn get_uuid(&self) -> String {
                String::from(#uuid)
            }

            fn get_path(&self, base_path: zbus::zvariant::ObjectPath<'static>) -> zbus::zvariant::ObjectPath<'static> {
                match zbus::zvariant::ObjectPath::from_static_str(#service_path) {
                    Ok(path) => path,
                    Err(_) => match base_path.as_str() {
                        "/" => zbus::zvariant::ObjectPath::from_string_unchecked(format!("{}{}", base_path, #name_str)),
                        _ => zbus::zvariant::ObjectPath::from_string_unchecked(format!("{}/{}", base_path, #name_str)),
                    }
                }
            }

            async fn register(
                mut self,
                bus: zbus::Connection,
                app_path: zbus::zvariant::ObjectPath<'static>,
            ) -> Result<bool> {
                println!("service register started");
                let characteristics = self.get_characteristics();
                let path = self.get_path(app_path);
                self.characteristic_paths = characteristics
                    .iter()
                    .map(|chrc| chrc.get_path(path.clone()))
                    .collect::<Vec<zbus::zvariant::ObjectPath<'_>>>();
                for chrc in characteristics {
                    chrc.register(bus.clone(), path.clone()).await?;
                    println!("chrc registered");
                }
                bus.object_server().at(path, self).await
            }
         }
    };
    gen.into()
}

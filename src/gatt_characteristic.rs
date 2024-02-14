use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{Data, DataStruct, Expr, ExprArray, ExprLit, Fields, FieldsNamed, Lit, Token};

use super::util::{get_valid_arg, ArgValueType};

pub struct GattCharacteristicArgs {
    pub uuid: Option<ExprLit>,
    pub flags: Option<ExprArray>,
    pub service: Option<ExprLit>,
    pub path: Option<ExprLit>,
}

impl GattCharacteristicArgs {
    fn new(value_map: HashMap<String, ArgValueType>, stream: ParseStream) -> Result<Self> {
        let mut uuid = None;
        let mut flags = None;
        let mut service = None;
        let mut path = None;

        if let ArgValueType::Str(Some(val)) = value_map.get("uuid").unwrap() {
            uuid = Some(val.to_owned());
        }
        if let ArgValueType::Str(Some(val)) = value_map.get("path").unwrap() {
            path = Some(val.to_owned());
        }
        if let ArgValueType::VecStr(val) = value_map.get("flags").unwrap() {
            flags = val.to_owned();
        }
        if let ArgValueType::Str(Some(val)) = value_map.get("service").unwrap() {
            service = Some(val.to_owned());
        }

        if let None = uuid {
            return Err(stream.error("uuid must be defined"));
        }

        Ok(Self {
            uuid,
            flags,
            service,
            path,
        })
    }
}

impl Parse for GattCharacteristicArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let expressions = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;

        let mut arg_map: HashMap<String, ArgValueType> = HashMap::from([
            ("uuid".into(), ArgValueType::Str(None)),
            ("flags".into(), ArgValueType::VecStr(None)),
            ("service".into(), ArgValueType::Str(None)),
            ("path".into(), ArgValueType::Str(None)),
        ]);

        for expr in expressions {
            let (arg_name, arg_value) = get_valid_arg(&arg_map, &expr, &input)?;
            let _ = arg_map.insert(arg_name, arg_value);
        }

        GattCharacteristicArgs::new(arg_map, input)
    }
}

pub fn apply_macro(
    ast: &syn::DeriveInput,
    uuid: ExprLit,
    flags: Option<ExprArray>,
    path: Option<ExprLit>,
) -> TokenStream {
    let name = &ast.ident;
    let name_str = name.clone().to_string();

    let visibility = &ast.vis;

    let extra_props;
    if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = &ast.data
    {
        extra_props = quote! { #named };
    } else {
        extra_props = quote! {};
    }

    let chrc_path = match path {
        Some(path) => quote! { #path },
        None => quote! { "" },
    };

    let chrc_flags = match &flags {
        Some(arr) => quote! { #arr },
        None => quote! { [] },
    };

    let (gatt_interface_methods, gatt_priv_methods) = generate_gatt_methods(&flags);

    let gen = quote! {
        use gattrs::*;
        use gattrs::zbus::*;

        #[derive(derivative::Derivative)]
        #[derivative(Default)]
        #visibility struct #name {
            service: Option<zbus::zvariant::ObjectPath<'static>>,
            notification_message: Vec<u8>,
            path: zbus::zvariant::ObjectPath<'static>,
            bus: Option<zbus::Connection>,

            #extra_props
        }

        #[dbus_interface(name = "org.bluez.GattCharacteristic1")]
        impl #name {
            #[dbus_interface(property, name = "UUID")]
            fn uuid(&self) -> &str {
                #uuid
            }

            #[dbus_interface(property)]
            fn flags(&self) -> Vec<&str> {
                vec!#chrc_flags
            }

            #[dbus_interface(property)]
            fn service(&self) -> zbus::zvariant::ObjectPath<'static> {
                dbg!(self.service.clone());
                self.service.clone().unwrap()
            }

            #[dbus_interface(property)]
            fn value(&self) -> &[u8] {
                self.notification_message.as_slice()
            }

            #gatt_interface_methods
         }

         impl #name {
            #gatt_priv_methods
         }

         #[gattrs::async_trait::async_trait]
         impl gattrs::gatt::CharacteristicRegister for #name {
            fn get_path(&self, service_path: zbus::zvariant::ObjectPath<'static>) -> zbus::zvariant::ObjectPath<'static> {
                match zbus::zvariant::ObjectPath::from_static_str(#chrc_path) {
                    Ok(path) => path,
                    Err(_) => zbus::zvariant::ObjectPath::from_string_unchecked(format!("{}/{}", service_path, #name_str))
                }
            }

            async fn register(
                mut self,
                bus: zbus::Connection,
                service_path: zbus::zvariant::ObjectPath<'static>,
            ) -> zbus::Result<bool> {
                let path = self.get_path(service_path.clone());
                self.path = path.clone();
                println!("{}", &service_path);
                self.service = Some(service_path);
                self.bus = Some(bus.clone());
                bus.object_server().at(path, self).await
            }
         }
    };
    gen.into()
}

fn generate_gatt_methods(flags: &Option<ExprArray>) -> (TokenStream, TokenStream) {
    let mut read_fn = quote! {};
    let mut write_fn = quote! {};
    let mut notify_fns = quote! {};
    let mut notify_priv_fn = quote! {};

    if let Some(ExprArray { elems, .. }) = flags {
        for expr in elems {
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(literal),
                ..
            }) = expr
            {
                let val = literal.value();
                if val.contains("read") {
                    read_fn = quote! {
                        async fn read_value(
                            &self,
                            _opts: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
                            #[zbus(connection)] bus: &zbus::Connection,
                        ) -> zbus::fdo::Result<Vec<u8>> {
                            println!("read");
                            self.read().await
                        }
                    };
                } else if val.contains("write") {
                    write_fn = quote! {
                        async fn write_value(
                            &mut self,
                            value: &[u8],
                            _opts: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
                            #[zbus(connection)] bus: &zbus::Connection,
                        ) -> zbus::fdo::Result<()> {
                            println!("write");
                            self.write(value).await
                        }
                    };
                } else if val.contains("notify") {
                    notify_fns = quote! {
                        async fn start_notify(
                            &self,
                            #[zbus(header)] _header: zbus::MessageHeader<'_>,
                            #[zbus(signal_context)] _ctxt: zbus::SignalContext<'_>,
                        ) -> zbus::fdo::Result<()> {
                            println!("notifications started");
                            Ok(())
                        }
                        async fn stop_notify(
                            &self,
                            #[zbus(header)] header: zbus::MessageHeader<'_>,
                            #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
                        ) -> zbus::fdo::Result<()> {
                            println!("notifications stopped");
                            Ok(())
                        }
                    };
                    notify_priv_fn = quote! {
                        async fn notify(
                            &mut self,
                            message: Vec<u8>,
                        ) -> zbus::fdo::Result<()> {
                            if let Some(bus) = self.bus.as_ref() {
                                let ctxt = zbus::SignalContext::new(bus, &self.path).unwrap();
                                self.notification_message = message;
                                self.value_changed(&ctxt).await?;
                            }
                            Ok(())
                        }
                    };
                } else {
                    panic!("Invalid characteristic flag \"{}\"", val);
                }
            }
        }
    }

    let interface_methods = quote! { #read_fn #write_fn #notify_fns };
    let priv_methods = quote! { #notify_priv_fn };

    (interface_methods.into(), priv_methods.into())
}

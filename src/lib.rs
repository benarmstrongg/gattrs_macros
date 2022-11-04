mod gatt_characteristic;
mod gatt_service;
mod util;

use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn gatt_service(metadata: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input::parse::<gatt_service::GattServiceArgs>(metadata).unwrap();
    let ast = syn::parse(input).unwrap();

    gatt_service::apply_macro(&ast, args.uuid.unwrap(), args.primary, args.path)
}

#[proc_macro_attribute]
pub fn gatt_characteristic(metadata: TokenStream, input: TokenStream) -> TokenStream {
    let args =
        parse_macro_input::parse::<gatt_characteristic::GattCharacteristicArgs>(metadata).unwrap();
    let ast = syn::parse(input).unwrap();

    gatt_characteristic::apply_macro(&ast, args.uuid.unwrap(), args.flags, args.path, args.paged).into()
}

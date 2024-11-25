mod define_amd64_registers;

#[proc_macro]
pub fn define_amd64_registers(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    define_amd64_registers::impl_define_registers(input.into())
        .unwrap()
        .into()
}

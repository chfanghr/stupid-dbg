use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream, Result},
    parse2, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    Arm, Error, Expr, ExprCall, Ident, ItemEnum, ItemFn, ItemImpl, Lit, Path, Token,
};

#[derive(Debug)]
struct Gpr64 {
    name: String,
    dwarf_id: Option<usize>,
}

#[derive(Debug)]
struct GprSub {
    name: String,
    base_name: String,
    byte_width: usize,
    offset_on_base: usize,
}

#[derive(Debug)]
struct Fpr {
    name: String,
    dwarf_id: Option<usize>,
    name_in_fpregs_struct: String,
}

#[derive(Debug)]
struct RId {
    id: usize,
}

#[derive(Debug)]
enum RegDef {
    Gpr64(Gpr64),
    GprSub(GprSub),
    Fpr(Fpr),
    FpSt(RId),
    FpMM(RId),
    FpXMM(RId),
    Dr(RId),
}

#[derive(Debug)]
struct RegDefs {
    defs: Vec<RegDef>,
}

fn parse_optional<Input, ParserFn, Output>(
    parse: ParserFn,
    input: Option<Input>,
) -> Result<Option<Output>>
where
    ParserFn: FnOnce(Input) -> Result<Output>,
{
    input.map(parse).transpose()
}

fn check_args_length_and_into_iter<T, P>(
    allowed_len: Vec<usize>,
    args: Punctuated<T, P>,
) -> Result<<Punctuated<T, P> as IntoIterator>::IntoIter>
where
    Punctuated<T, P>: Spanned,
{
    if allowed_len.contains(&args.len()) {
        Ok(args.into_iter())
    } else {
        Err(Error::new(
            args.span(),
            format!(
                "directive function accepts the following number of arguments: {:?}",
                allowed_len
            ),
        ))
    }
}

fn parse_name(expr: Expr) -> Result<String> {
    let name = match expr {
        Expr::Path(p) => Ok(p.path),
        expr => Err(Error::new(expr.span(), "name must be an ident")),
    }?;
    let name = match name.get_ident() {
        Some(ident) => Ok(ident.clone()),
        None => Err(Error::new(name.span(), "name must be an single ident")),
    }?;
    Ok(name.to_string())
}

fn parse_register_name(expr: Expr) -> Result<String> {
    parse_name(expr)
}

fn parse_name_in_fpregs_struct(expr: Expr) -> Result<String> {
    parse_name(expr)
}

fn parse_usize_lit(expr: Expr) -> Result<usize> {
    let lit = match expr {
        Expr::Lit(lit) => Ok(lit.lit),
        expr => Err(Error::new(expr.span(), "expr must be a lit")),
    }?;

    match lit {
        Lit::Int(lit_int) => lit_int.base10_parse::<usize>(),
        lit => Err(Error::new(lit.span(), "lit must be an unsigned integer")),
    }
}

fn parse_dwarf_id(expr: Expr) -> Result<usize> {
    parse_usize_lit(expr)
}

fn parse_register_id(expr: Expr) -> Result<usize> {
    parse_usize_lit(expr)
}

fn parse_gpr_64(args: Punctuated<Expr, Comma>) -> Result<Gpr64> {
    let mut args_iter = check_args_length_and_into_iter(vec![1, 2], args)?;

    let name = parse_register_name(args_iter.next().unwrap())?;
    let dwarf_id = parse_optional(parse_dwarf_id, args_iter.next())?;

    Ok(Gpr64 { name, dwarf_id })
}

fn parse_gpr_sub(
    byte_width: usize,
    offset_on_base: usize,
    args: Punctuated<Expr, Comma>,
) -> Result<GprSub> {
    let mut args_iter = check_args_length_and_into_iter(vec![2], args)?;

    let name = parse_register_name(args_iter.next().unwrap())?;
    let base_name = parse_register_name(args_iter.next().unwrap())?;

    Ok(GprSub {
        name,
        base_name,
        byte_width,
        offset_on_base,
    })
}

fn parse_fpr(args: Punctuated<Expr, Comma>) -> Result<Fpr> {
    let mut args_iter = check_args_length_and_into_iter(vec![2, 3], args)?;

    let name = parse_register_name(args_iter.next().unwrap())?;
    let name_in_fpregs_struct = parse_name_in_fpregs_struct(args_iter.next().unwrap())?;
    let dwarf_id = parse_optional(parse_dwarf_id, args_iter.next())?;

    Ok(Fpr {
        name,
        dwarf_id,
        name_in_fpregs_struct,
    })
}

fn parse_r_id(args: Punctuated<Expr, Comma>) -> Result<RId> {
    let mut args_iter = check_args_length_and_into_iter(vec![1, 3], args)?;
    let id = parse_register_id(args_iter.next().unwrap())?;
    Ok(RId { id })
}

// gpr_64(<name>, <dwarf id>?)
// gpr_(8l|8h|16|32)(<name>, <base name>)
// fpr(<name>, <name in fpregs struct>, <dwarf id>?)
// fp_st(<id>)
// fp_mm(<id>)
// fp_xmm(<id>)
// dr(<id>)
impl Parse for RegDef {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let expr_call = ExprCall::parse(input)?;

        let directive = match *expr_call.func {
            Expr::Path(path) => Ok(path.path),
            expr => Err(Error::new(
                expr.span(),
                "register directive function should be a path",
            )),
        }?;
        let directive = match directive.get_ident() {
            Some(ident) => Ok(ident),
            _ => Err(Error::new(
                directive.span(),
                "register directive function path be a single ident",
            )),
        }?;

        let args = expr_call.args;

        match directive.to_string().as_str() {
            "gpr_64" => parse_gpr_64(args).map(RegDef::Gpr64),
            "gpr_32" => parse_gpr_sub(4, 0, args).map(RegDef::GprSub),
            "gpr_16" => parse_gpr_sub(2, 0, args).map(RegDef::GprSub),
            "gpr_8l" => parse_gpr_sub(1, 0, args).map(RegDef::GprSub),
            "gpr_8h" => parse_gpr_sub(1, 1, args).map(RegDef::GprSub),
            "fpr" => parse_fpr(args).map(RegDef::Fpr),
            "fp_st" => parse_r_id(args).map(RegDef::FpSt),
            "fp_mm" => parse_r_id(args).map(RegDef::FpMM),
            "fp_xmm" => parse_r_id(args).map(RegDef::FpXMM),
            "dr" => parse_r_id(args).map(RegDef::Dr),
            _ => Err(Error::new(
                directive.span(),
                format!("unknown register directive: {}", directive),
            )),
        }
    }
}

impl Parse for RegDefs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let punctuated = Punctuated::<RegDef, Token![;]>::parse_terminated(input)?;
        let registers = punctuated.into_iter().collect::<Vec<_>>();
        Ok(Self { defs: registers })
    }
}

impl RegDef {
    fn name(&self) -> String {
        match self {
            RegDef::Gpr64(gpr64) => gpr64.name.clone(),
            RegDef::GprSub(gpr_sub) => gpr_sub.name.clone(),
            RegDef::Fpr(fpr) => fpr.name.clone(),
            RegDef::FpSt(rid) => format!("st{}", rid.id),
            RegDef::FpMM(rid) => format!("mm{}", rid.id),
            RegDef::FpXMM(rid) => format!("xmm{}", rid.id),
            RegDef::Dr(rid) => format!("dr{}", rid.id),
        }
    }

    fn name_expr(&self) -> Expr {
        let name = self.name();

        parse_quote!(#name)
    }

    fn enum_variant_ident(&self) -> Ident {
        format_ident!(
            "{}",
            self.name().from_case(Case::Flat).to_case(Case::Pascal)
        )
    }

    fn dwarf_id_expr(&self) -> Expr {
        match self {
            RegDef::Gpr64(Gpr64 { dwarf_id, .. }) | RegDef::Fpr(Fpr { dwarf_id, .. }) => {
                match dwarf_id {
                    Some(dwarf_id) => parse_quote!(Some(#dwarf_id)),
                    None => parse_quote!(None),
                }
            }
            RegDef::GprSub(_) => parse_quote!(None),
            RegDef::FpSt(RId { id }) => parse_quote!(Some(33usize + #id)),
            RegDef::FpMM(RId { id }) => parse_quote!(Some(41usize + #id)),
            RegDef::FpXMM(RId { id }) => parse_quote!(Some(17usize + #id)),
            RegDef::Dr(_) => parse_quote!(None),
        }
    }

    fn final_offset_expr(&self) -> Expr {
        match self {
            RegDef::Gpr64(_) => parse_quote!(0usize),
            RegDef::GprSub(GprSub { offset_on_base, .. }) => parse_quote!(#offset_on_base),
            RegDef::Fpr(_) => parse_quote!(0usize),
            RegDef::FpSt(RId { id }) | RegDef::FpMM(RId { id }) | RegDef::FpXMM(RId { id }) => {
                parse_quote!(#id * 16usize)
            }
            RegDef::Dr(RId { id }) => parse_quote!(#id * 8usize),
        }
    }

    fn type_of_field_in_user_sub_struct(&self) -> Option<Path> {
        match &self {
            RegDef::Gpr64(_) | RegDef::GprSub(_) => Some(parse_quote!(libc::user_regs_struct)),
            RegDef::Fpr(_) | RegDef::FpSt(_) | RegDef::FpMM(_) | RegDef::FpXMM(_) => {
                Some(parse_quote!(libc::user_fpregs_struct))
            }

            RegDef::Dr(_) => None,
        }
    }

    fn field_in_user_sub_struct_ident(&self) -> Option<Ident> {
        match &self {
            RegDef::Gpr64(gpr64) => Some(format_ident!("{}", gpr64.name)),
            RegDef::GprSub(gpr_sub) => Some(format_ident!("{}", gpr_sub.base_name)),
            RegDef::Fpr(fpr) => Some(format_ident!("{}", fpr.name_in_fpregs_struct)),
            RegDef::FpSt(_) | RegDef::FpMM(_) => Some(format_ident!("st_space")),
            RegDef::FpXMM(_) => Some(format_ident!("xmm_space")),
            RegDef::Dr(_) => None,
        }
    }

    fn field_in_user_struct_ident(&self) -> Ident {
        match self {
            RegDef::Gpr64(_) => format_ident!("regs"),
            RegDef::GprSub(_) => format_ident!("regs"),
            RegDef::Fpr(_) => format_ident!("i387"),
            RegDef::FpSt(_) => format_ident!("i387"),
            RegDef::FpMM(_) => format_ident!("i387"),
            RegDef::FpXMM(_) => format_ident!("i387"),
            RegDef::Dr(_) => format_ident!("u_debugreg"),
        }
    }

    fn offset_in_user_struct_expr(&self) -> Expr {
        let first_level_field = self.field_in_user_struct_ident();
        let first_level_offset_expr: Expr =
            parse_quote!(core::mem::offset_of!(libc::user, #first_level_field));

        let second_level_field_type = self.type_of_field_in_user_sub_struct();
        let second_level_field = self.field_in_user_sub_struct_ident();
        let second_level_offset_expr: Expr = match second_level_field.zip(second_level_field_type) {
            Some((second_level, second_level_type)) => {
                parse_quote!(core::mem::offset_of!(#second_level_type, #second_level))
            }
            None => parse_quote!(0usize),
        };
        let final_offset_expr = self.final_offset_expr();

        parse_quote!(#first_level_offset_expr + #second_level_offset_expr + #final_offset_expr)
    }

    /*
    enum RegisterKind {
        GeneralPurpose,
        SubGeneralPurpose,
        FloatingPoint,
        Debug,
    }
    */
    fn kind_expr(&self) -> Expr {
        match self {
            RegDef::Gpr64(_) => parse_quote!(RegisterKind::GeneralPurpose),
            RegDef::GprSub(_) => parse_quote!(RegisterKind::SubGeneralPurpose),
            RegDef::Fpr(_) | RegDef::FpSt(_) | RegDef::FpMM(_) | RegDef::FpXMM(_) => {
                parse_quote!(RegisterKind::FloatingPoint)
            }
            RegDef::Dr(_) => parse_quote!(RegisterKind::Debug),
        }
    }

    /*
    enum RegisterRepr{
        UInt,
        LongDouble,
        Vector
    }
     */
    fn repr_expr(&self) -> Expr {
        match self {
            RegDef::Gpr64(_) | RegDef::GprSub(_) => parse_quote!(RegisterRepr::UInt),
            RegDef::Fpr(_) => parse_quote!(RegisterRepr::UInt),
            RegDef::FpSt(_) => parse_quote!(RegisterRepr::LongDouble),
            RegDef::FpMM(_) | RegDef::FpXMM(_) => {
                parse_quote!(RegisterRepr::Vector)
            }
            RegDef::Dr(_) => parse_quote!(RegisterRepr::UInt),
        }
    }

    fn byte_width_expr(&self) -> Expr {
        match self {
            RegDef::Gpr64(_) => parse_quote!(8usize),
            RegDef::GprSub(GprSub { byte_width, .. }) => parse_quote!(#byte_width),
            RegDef::Fpr(Fpr {
                name_in_fpregs_struct,
                ..
            }) => {
                let name_in_fpregs_struct = format_ident!("{}", name_in_fpregs_struct);
                parse_quote!(field_size!(libc::user_regs_struct, #name_in_fpregs_struct))
            }
            RegDef::FpSt(_) => parse_quote!(16usize),
            RegDef::FpMM(_) => parse_quote!(8usize),
            RegDef::FpXMM(_) => parse_quote!(16usize),
            RegDef::Dr(_) => parse_quote!(8usize),
        }
    }
}

// Stolen from https://stackoverflow.com/a/70222282
fn field_size_macro() -> TokenStream {
    quote! {
        macro_rules! field_size {
            ($t:path, $field:ident) => {{
                let m = core::mem::MaybeUninit::<$t>::uninit();
                let p = unsafe {
                    core::ptr::addr_of!((*(&m as *const _ as *const $t)).$field)
                };

                const fn size_of_raw<T>(_: *const T) -> usize {
                    core::mem::size_of::<T>()
                }
                size_of_raw(p)
            }};
        }
    }
}

impl RegDefs {
    fn item_enum(&self) -> ItemEnum {
        let variants = self.defs.iter().map(|def| def.enum_variant_ident());
        parse_quote!(
            #[repr(u8)]
            #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
            pub enum Register {
              #(#variants),*
            }
        )
    }

    fn offset_in_user_struct_item_fn(&self) -> ItemFn {
        let arms = self.defs.iter().map(|def| -> Arm {
            let variant = def.enum_variant_ident();
            let expr = def.offset_in_user_struct_expr();
            parse_quote!(Self::#variant => #expr)
        });

        parse_quote!(
            pub fn offset_in_user_struct(&self) -> usize {
                match self {
                    #(#arms),*
                }
            }
        )
    }

    fn dwarf_id_item_fn(&self) -> ItemFn {
        let arms = self.defs.iter().map(|def| -> Arm {
            let variant = def.enum_variant_ident();
            let expr = def.dwarf_id_expr();
            parse_quote!(Self::#variant => #expr)
        });

        parse_quote!(
            pub fn dwarf_id(&self) -> Option<usize> {
                match self {
                    #(#arms),*
                }
            }
        )
    }

    fn name_item_fn(&self) -> ItemFn {
        let arms = self.defs.iter().map(|def| -> Arm {
            let variant = def.enum_variant_ident();
            let expr = def.name_expr();
            parse_quote!(Self::#variant => #expr)
        });

        parse_quote!(
            pub fn name(&self) -> &'static str {
                match self {
                    #(#arms),*
                }
            }
        )
    }

    fn all_variants_item_fn(&self) -> ItemFn {
        let variant_count = self.defs.len();
        let variants = self.defs.iter().map(|def| -> Expr {
            let variant = def.enum_variant_ident();
            parse_quote!(Self::#variant)
        });

        parse_quote!(
            pub fn all_variants() -> [Self;#variant_count] {
                [
                    #(#variants),*
                ]
            }
        )
    }

    fn kind_fn_item(&self) -> ItemFn {
        let arms = self.defs.iter().map(|def| -> Arm {
            let variant = def.enum_variant_ident();
            let expr = def.kind_expr();
            parse_quote!(Self::#variant => #expr)
        });

        parse_quote!(
            pub fn kind(&self) -> RegisterKind {
                match self {
                    #(#arms),*
                }
            }
        )
    }

    fn repr_fn_item(&self) -> ItemFn {
        let arms = self.defs.iter().map(|def| -> Arm {
            let variant = def.enum_variant_ident();
            let expr = def.repr_expr();
            parse_quote!(Self::#variant => #expr)
        });

        parse_quote!(
            pub fn repr(&self) -> RegisterRepr {
                match self {
                    #(#arms),*
                }
            }
        )
    }

    fn byte_width_fn_item(&self) -> ItemFn {
        let field_size_macro = field_size_macro();
        let arms = self.defs.iter().map(|def| -> Arm {
            let variant = def.enum_variant_ident();
            let expr = def.byte_width_expr();
            parse_quote!(Self::#variant => #expr)
        });

        parse_quote!(
            pub fn byte_width(&self) -> usize {
                #[allow(unused)]
                #field_size_macro

                match self {
                    #(#arms),*
                }
            }
        )
    }

    fn item_impl(&self) -> ItemImpl {
        let offset_in_user_struct_member_fn = self.offset_in_user_struct_item_fn();
        let dwarf_id_item_fn = self.dwarf_id_item_fn();
        let name_item_fn = self.name_item_fn();
        let all_variants_item_fn = self.all_variants_item_fn();
        let kind_fn_item = self.kind_fn_item();
        let repr_fn_item = self.repr_fn_item();
        let byte_width_fn_item = self.byte_width_fn_item();

        parse_quote!(
            impl Register{
                #offset_in_user_struct_member_fn
                #dwarf_id_item_fn
                #name_item_fn
                #all_variants_item_fn
                #kind_fn_item
                #repr_fn_item
                #byte_width_fn_item
            }
        )
    }
}

impl ToTokens for RegDefs {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.item_enum().to_tokens(tokens);
        self.item_impl().to_tokens(tokens);
    }
}

pub(crate) fn impl_define_registers(
    input: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    parse2::<RegDefs>(input).map(|defs| defs.to_token_stream())
}

#[test]
fn playground() {
    let input = quote! {
      gpr_64(rax, 0);
      gpr_32(ebp, rbp);
      gpr_8h(ah, rax);
      fpr(fcw, cwd, 65);
      fpr(frip, rip);
      fpr(mxcsrmask, mxcr_mask);
      fp_st(0);
      fp_mm(0);
      fp_xmm(0);
      dr(0);
    };

    let output = impl_define_registers(input).unwrap();
    let file = syn::parse_file(&output.to_string()).unwrap();
    let pp_output = prettyplease::unparse(&file);

    print!("{}", pp_output);
}

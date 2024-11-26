use std::{cmp::PartialEq, collections::BTreeMap, iter, mem::MaybeUninit};

use anyhow::anyhow;
use helper_proc_macros::define_amd64_registers;
use lazy_static::lazy_static;
use nix::{sys::ptrace, unistd::Pid};
use tracing::debug;

use crate::aux::{ptrace_getfpregs, read_any_from_void_pointer};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegisterKind {
    GeneralPurpose,
    SubGeneralPurpose,
    FloatingPoint,
    Debug,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegisterRepr {
    UInt,
    LongDouble,
    Vector,
}

define_amd64_registers! {
    // gpr_64(<name>, <dwarf_id>)
    gpr_64(rax, 0);
    gpr_64(rdx, 1);
    gpr_64(rcx, 2);
    gpr_64(rbx, 3);
    gpr_64(rsi, 4);
    gpr_64(rdi, 5);
    gpr_64(rbp, 6);
    gpr_64(rsp, 7);
    gpr_64(r8, 8);
    gpr_64(r9, 9);
    gpr_64(r10, 10);
    gpr_64(r11, 11);
    gpr_64(r12, 12);
    gpr_64(r13, 13);
    gpr_64(r14, 14);
    gpr_64(r15, 15);
    gpr_64(rip, 16);
    gpr_64(eflags, 49);
    gpr_64(es, 50);
    gpr_64(cs, 51);
    gpr_64(ss, 52);
    gpr_64(ds, 53);
    gpr_64(fs, 54);
    gpr_64(gs, 55);
    gpr_64(orig_rax);

    // gpr_32(<name>, <base_name>)
    gpr_32(eax, rax);
    gpr_32(ebx, rbx);
    gpr_32(ecx, rcx);
    gpr_32(edx, rdx);
    gpr_32(esi, rsi);
    gpr_32(edi, rdi);
    gpr_32(ebp, rbp);
    gpr_32(esp, rsp);
    gpr_32(r8d, r8);
    gpr_32(r9d, r9);
    gpr_32(r10d, r10);
    gpr_32(r11d, r11);
    gpr_32(r12d, r12);
    gpr_32(r13d, r13);
    gpr_32(r14d, r14);
    gpr_32(r15d, r15);

    // gpr_16(<name>, <base_name>)
    gpr_16(ax, rax);
    gpr_16(bx, rbx);
    gpr_16(cx, rcx);
    gpr_16(dx, rdx);
    gpr_16(si, rsi);
    gpr_16(di, rdi);
    gpr_16(bp, rbp);
    gpr_16(sp, rsp);
    gpr_16(r8w, r8);
    gpr_16(r9w, r9);
    gpr_16(r10w, r10);
    gpr_16(r11w, r11);
    gpr_16(r12w, r12);
    gpr_16(r13w, r13);
    gpr_16(r14w, r14);
    gpr_16(r15w, r15);

    // gpr_8l(<name>, <base_name>)
    gpr_8l(al, rax);
    gpr_8l(bl, rbx);
    gpr_8l(cl, rcx);
    gpr_8l(dl, rdx);
    gpr_8l(sil, rsi);
    gpr_8l(dil, rdi);
    gpr_8l(bpl, rbp);
    gpr_8l(spl, rsp);
    gpr_8l(r8b, r8);
    gpr_8l(r9b, r9);
    gpr_8l(r10b, r10);
    gpr_8l(r11b, r11);
    gpr_8l(r12b, r12);
    gpr_8l(r13b, r13);
    gpr_8l(r14b, r14);
    gpr_8l(r15b, r15);

    // gpr_8h(<name>, <base_name>)
    gpr_8h(ah, rax);
    gpr_8h(bh, rbx);
    gpr_8h(ch, rcx);
    gpr_8h(dd, rdx);

    // fpr(<name>, <field_name_in_user_fpregs_struct>,<dwarf_id>)
    fpr(fcw, cwd, 65);
    fpr(fsw, swd, 66);
    fpr(ftw, ftw);
    fpr(fop, fop);
    fpr(frip, rip);
    fpr(frdp, rdp);
    fpr(mxcsr, mxcsr, 64);
    fpr(mxcsrmask, mxcr_mask);

    // fp_st(<id>)
    fp_st(0);
    fp_st(1);
    fp_st(2);
    fp_st(3);
    fp_st(4);
    fp_st(5);
    fp_st(6);
    fp_st(7);

    // fp_mm(<id>)
    fp_mm(0);
    fp_mm(1);
    fp_mm(2);
    fp_mm(3);
    fp_mm(4);
    fp_mm(5);
    fp_mm(6);
    fp_mm(7);

    // fp_xmm(<id>)
    fp_xmm(0);
    fp_xmm(1);
    fp_xmm(2);
    fp_xmm(3);
    fp_xmm(4);
    fp_xmm(5);
    fp_xmm(6);
    fp_xmm(7);
    fp_xmm(8);
    fp_xmm(9);
    fp_xmm(10);
    fp_xmm(11);
    fp_xmm(12);
    fp_xmm(13);
    fp_xmm(14);
    fp_xmm(15);

    // dr(<id>)
    dr(0);
    dr(1);
    dr(2);
    dr(3);
    dr(4);
    dr(5);
    dr(6);
    dr(7);
}

lazy_static! {
    static ref NAME_TO_REGISTER_MAP: BTreeMap<&'static str, Register> = Register::all_registers()
        .into_iter()
        .map(|reg| (reg.name(), reg))
        .collect();
    static ref DWARF_ID_TO_REGISTER_MAP: BTreeMap<usize, Register> = Register::all_registers()
        .into_iter()
        .filter_map(|reg| reg.dwarf_id().map(|id| (id, reg)))
        .collect();
}

impl Register {
    pub fn lookup_by_name(name: &str) -> Option<Register> {
        NAME_TO_REGISTER_MAP.get(name).copied()
    }

    pub fn lookup_by_dwarf_id(dwarf_id: usize) -> Option<Register> {
        DWARF_ID_TO_REGISTER_MAP.get(&dwarf_id).copied()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RegisterValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F128(f128),
    Byte64([u8; 8]),
    Byte128([u8; 16]),
}

impl RegisterValue {
    fn byte_width(&self) -> usize {
        match self {
            RegisterValue::U8(x) => size_of_val(x),
            RegisterValue::U16(x) => size_of_val(x),
            RegisterValue::U32(x) => size_of_val(x),
            RegisterValue::U64(x) => size_of_val(x),
            RegisterValue::I8(x) => size_of_val(x),
            RegisterValue::I16(x) => size_of_val(x),
            RegisterValue::I32(x) => size_of_val(x),
            RegisterValue::I64(x) => size_of_val(x),
            RegisterValue::F128(x) => size_of_val(x),
            RegisterValue::Byte64(x) => size_of_val(x),
            RegisterValue::Byte128(x) => size_of_val(x),
        }
    }

    unsafe fn as_u8_ptr(&self) -> *const u8 {
        match self {
            RegisterValue::U8(x) => x,
            RegisterValue::U16(x) => (x as *const u16).cast(),
            RegisterValue::U32(x) => (x as *const u32).cast(),
            RegisterValue::U64(x) => (x as *const u64).cast(),
            RegisterValue::I8(x) => (x as *const i8).cast(),
            RegisterValue::I16(x) => (x as *const i16).cast(),
            RegisterValue::I32(x) => (x as *const i32).cast(),
            RegisterValue::I64(x) => (x as *const i64).cast(),
            RegisterValue::F128(x) => (x as *const f128).cast(),
            RegisterValue::Byte64(x) => (x as *const [u8; 8]).cast(),
            RegisterValue::Byte128(x) => (x as *const [u8; 16]).cast(),
        }
    }
}

impl Register {
    unsafe fn get_ptr_in_user_struct(&self, user: &libc::user) -> *const u8 {
        let offset = self.offset_in_user_struct();
        let ptr: *const libc::user = user;
        let ptr = ptr.cast::<u8>();
        let ptr = unsafe { ptr.offset(offset.try_into().unwrap()) };
        return ptr;
    }

    unsafe fn get_mut_ptr_in_user_struct(&self, user: &mut libc::user) -> *mut u8 {
        let offset = self.offset_in_user_struct();
        let ptr: *mut libc::user = user;
        let ptr = ptr.cast::<u8>();
        let ptr = unsafe { ptr.offset(offset.try_into().unwrap()) };
        return ptr;
    }

    pub fn read_from_user_struct(&self, user: &libc::user) -> anyhow::Result<RegisterValue> {
        let byte_width = self.byte_width();
        let repr = self.repr();

        let ptr = unsafe { self.get_ptr_in_user_struct(user) };

        let val = match (repr, byte_width) {
            (RegisterRepr::UInt, 1) => {
                RegisterValue::U8(unsafe { read_any_from_void_pointer(ptr, 1) })
            }
            (RegisterRepr::UInt, 2) => {
                RegisterValue::U16(unsafe { read_any_from_void_pointer(ptr, 2) })
            }
            (RegisterRepr::UInt, 4) => {
                RegisterValue::U32(unsafe { read_any_from_void_pointer(ptr, 4) })
            }
            (RegisterRepr::UInt, 8) => {
                RegisterValue::U64(unsafe { read_any_from_void_pointer(ptr, 8) })
            }
            (RegisterRepr::LongDouble, byte_width) => {
                RegisterValue::F128(unsafe { read_any_from_void_pointer(ptr, byte_width) })
            }
            (RegisterRepr::Vector, byte_width) => {
                if byte_width <= 8 {
                    RegisterValue::Byte64(unsafe { read_any_from_void_pointer(ptr, byte_width) })
                } else if byte_width <= 16 {
                    RegisterValue::Byte128(unsafe { read_any_from_void_pointer(ptr, byte_width) })
                } else {
                    unreachable!("register {:?}: vector is longer than 128 bit", self)
                }
            }
            (repr, byte_width) => {
                unreachable!(
                    "register {:?}: unhandled repr/byte width combination: {:?}, {:?}, ",
                    self, repr, byte_width,
                )
            }
        };

        Ok(val)
    }

    unsafe fn copy_to_user_struct(
        &self,
        from_ptr: *const u8,
        to_user: &mut libc::user,
        value_byte_width: usize,
    ) -> anyhow::Result<()> {
        let byte_width = self.byte_width();
        if self.byte_width() < value_byte_width {
            return Err(anyhow!("register {:?}: value doesn't fit in the register, value width: {}, register width: {}", self, value_byte_width, byte_width));
        }
        let ptr = self.get_mut_ptr_in_user_struct(to_user);
        let zeroed = iter::repeat(0u8).take(byte_width).collect::<Vec<u8>>();
        let zeroed_ptr = zeroed.as_ptr();
        zeroed_ptr.copy_to(ptr, byte_width);
        from_ptr.copy_to(ptr, value_byte_width);
        Ok(())
    }

    pub fn write_to_user_struct(
        &self,
        user: &mut libc::user,
        value: RegisterValue,
    ) -> anyhow::Result<()> {
        let value_byte_width = value.byte_width();
        unsafe {
            let value_ptr = value.as_u8_ptr();
            self.copy_to_user_struct(value_ptr, user, value_byte_width)
        }
    }

    pub unsafe fn write_any_to_user_struct<T>(
        &self,
        user: &mut libc::user,
        value: &T,
    ) -> anyhow::Result<()>
    where
        T: Sized,
    {
        let value_byte_width = size_of_val(value);
        let value_ptr = (value as *const T).cast::<u8>();
        self.copy_to_user_struct(value_ptr, user, value_byte_width)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Registers {
    user: libc::user,
}

impl Registers {
    pub fn read_register(&self, register: Register) -> anyhow::Result<RegisterValue> {
        register.read_from_user_struct(&self.user)
    }

    pub fn write_register(
        &mut self,
        register: Register,
        value: RegisterValue,
    ) -> anyhow::Result<()> {
        register.write_to_user_struct(&mut self.user, value)
    }

    pub unsafe fn write_register_any<T: Sized>(
        &mut self,
        register: Register,
        value: &T,
    ) -> anyhow::Result<()> {
        register.write_any_to_user_struct(&mut self.user, value)
    }

    pub fn read_with_ptrace(pid: Pid) -> anyhow::Result<Self> {
        debug!("calling ptrace::getregs");

        let mut user = unsafe { MaybeUninit::<libc::user>::zeroed().assume_init() };

        debug!("reading user registers");
        user.regs = ptrace::getregs(pid)?;

        debug!("reading floating point registers");
        user.i387 = ptrace_getfpregs(pid)?;

        for (idx, reg) in iter::zip(0usize..=8, Register::all_debug_registers()) {
            let offset = reg.offset_in_user_struct();
            debug!("reading debug register {:?}", reg);
            let reg_val = ptrace::read_user(pid, offset as *mut libc::c_void)?;
            user.u_debugreg[idx] = reg_val as u64;
        }

        Ok(Self { user })
    }
}

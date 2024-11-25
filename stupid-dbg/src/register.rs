use helper_proc_macros::define_amd64_registers;

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

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

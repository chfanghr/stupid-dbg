use std::{iter, mem::MaybeUninit};

use stupid_dbg::register::{Register, RegisterValue};

fn assert_read_register_value(
    register: Register,
    expected_value: RegisterValue,
    user: &libc::user,
) {
    let actual_value = register.read_from_user_struct(user).unwrap();
    if actual_value.ne(&expected_value) {
        panic!(
            "register: {:?}: unexpected register value, expected: {:?}, actual: {:?}",
            register, expected_value, actual_value
        )
    }
}

fn write_and_check_register_value(register: Register, value: RegisterValue, user: &mut libc::user) {
    register.write_to_user_struct(user, value).unwrap();
    assert_read_register_value(register, value, user);
}

unsafe fn write_any_and_check_register_value<T: Sized>(
    register: Register,
    value: T,
    expected_value: RegisterValue,
    user: &mut libc::user,
) {
    register.write_any_to_user_struct(user, &value).unwrap();
    assert_read_register_value(register, expected_value, user);
}

#[test]
fn read_and_write_registers() {
    let mut user = unsafe { MaybeUninit::<libc::user>::zeroed().assume_init() };
    assert_read_register_value(Register::Rax, RegisterValue::U64(0), &user);
    assert_read_register_value(Register::R15d, RegisterValue::U32(0), &user);
    assert_read_register_value(
        Register::Xmm0,
        RegisterValue::Byte128(
            iter::repeat(0u8)
                .take(16)
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
        ),
        &user,
    );

    write_and_check_register_value(Register::Rax, RegisterValue::U64(1), &mut user);
    write_and_check_register_value(Register::R15d, RegisterValue::U32(2), &mut user);
    write_and_check_register_value(
        Register::Xmm0,
        RegisterValue::Byte128(
            iter::repeat(42u8)
                .take(16)
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
        ),
        &mut user,
    );

    unsafe {
        write_any_and_check_register_value(
            Register::R15d,
            69u8,
            RegisterValue::U32(69u32),
            &mut user,
        );
        write_any_and_check_register_value(
            Register::R15d,
            69u16,
            RegisterValue::U32(69u32),
            &mut user,
        );
        write_any_and_check_register_value(
            Register::R15d,
            69u32,
            RegisterValue::U32(69u32),
            &mut user,
        );
    }
}

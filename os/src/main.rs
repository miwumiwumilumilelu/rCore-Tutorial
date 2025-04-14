// #![no_std]
// #![no_main]
// mod lang_items;

// use core::fmt::{self, Write};

// const SYSCALL_EXIT: usize = 93;

// fn syscall(id: usize, args: [usize; 3]) -> isize {
//     let mut ret;
//     unsafe {
//         core::arch::asm!(
//             "ecall",
//             inlateout("x10") args[0] => ret,
//             in("x11") args[1],
//             in("x12") args[2],
//             in("x17") id,
//         );
//     }
//     ret
// }

// pub fn sys_exit(xstate: i32) -> isize {
//     syscall(SYSCALL_EXIT, [xstate as usize, 0, 0])
// }


// //首先封装一下对 SYSCALL_WRITE 系统调用
// const SYSCALL_WRITE: usize = 64;

// pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
//   syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
// }

//然后实现基于 Write Trait 的数据结构，
//完成 Write Trait 所需要的 write_str 函数，并用 print 函数进行包装
// struct Stdout;

// impl Write for Stdout {
//     fn write_str(&mut self, s: &str) -> fmt::Result {
//         sys_write(1, s.as_bytes());
//         Ok(())
//     }
// }

// pub fn print(args: fmt::Arguments) {
//     Stdout.write_fmt(args).unwrap();
// }

// //最后，实现基于 print 函数，实现Rust语言格式化宏 
// #[macro_export]
// macro_rules! print {
//     ($fmt: literal $(, $($arg: tt)+)?) => {
//         $crate::console::print(format_args!($fmt $(, $($arg)+)?));
//     }
// }

// //实现 println! 宏，println! 宏的实现和 print! 宏类似，只是多了一个换行符
// #[macro_export]
// macro_rules! println {
//     ($fmt: literal $(, $($arg: tt)+)?) => {
//         $crate::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
//     }
// }
// mod sbi;
// use sbi::shutdown;
// //接下来，我们调整一下应用程序，让它发出显示字符串和退出的请求
// core::arch::global_asm!(include_str!("entry.asm"));

// fn clear_bss() {
//     unsafe extern "C" {
//         unsafe fn sbss();
//         unsafe fn ebss();
//     }
//     (sbss as usize..ebss as usize).for_each(|a| {
//         unsafe { (a as *mut u8).write_volatile(0) }
//     });
// }

// #[unsafe(no_mangle)]
// pub fn rust_main() -> ! {
//     clear_bss();
//     shutdown();
// }


//! The main module and entrypoint
//!
//! The operating system and app also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality [`clear_bss()`]. (See its source code for
//! details.)
//!
//! We then call [`println!`] to display `Hello, world!`.



//! The main module and entrypoint
//!
//! The operating system and app also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality [`clear_bss()`]. (See its source code for
//! details.)
//!
//! We then call [`println!`] to display `Hello, world!`.

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![no_main]


use core::arch::global_asm;
use log::*;

#[macro_use]
mod console;
mod lang_items;
mod logging;
mod sbi;

#[path = "boards/qemu.rs"]
mod board;

global_asm!(include_str!("entry.asm"));

/// clear BSS segment
pub fn clear_bss() {
    unsafe extern "C" {
        unsafe fn sbss();
        unsafe fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}

/// the rust entry-point of os
#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    unsafe extern "C" {
        unsafe fn stext(); // begin addr of text segment
        unsafe fn etext(); // end addr of text segment
        unsafe fn srodata(); // start addr of Read-Only data segment
        unsafe fn erodata(); // end addr of Read-Only data ssegment
        unsafe fn sdata(); // start addr of data segment
        unsafe fn edata(); // end addr of data segment
        unsafe fn sbss(); // start addr of BSS segment
        unsafe fn ebss(); // end addr of BSS segment
        unsafe fn boot_stack_lower_bound(); // stack lower bound
        unsafe fn boot_stack_top(); // stack top
    }
    clear_bss();
    logging::init();
    println!("[kernel] Hello, world!");
    trace!(
        "[kernel] .text [{:#x}, {:#x})",
        stext as usize,
        etext as usize
    );
    debug!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );
    info!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );
    warn!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as usize, boot_stack_lower_bound as usize
    );
    error!("[kernel] .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);

    use crate::board::QEMUExit;
    crate::board::QEMU_EXIT_HANDLE.exit_success(); // CI autotest success
                                                   //crate::board::QEMU_EXIT_HANDLE.exit_failure(); // CI autoest failed
}

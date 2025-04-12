-------------------------------------------------------------------
# ch1:应用程序与基本执行环境
目标:让应用与硬件隔离(操作系统的主要功能)
解决:如何设计和实现建立在裸机上的执行环境，并让应用程序能够在这样的执行环境中运行
-------------------------------------------------------------------
LOG=TRACE make run     指定LOG级别为TRACE，查看重要程度不低于TRACE的输出日志
-------------------------------------------------------------------
1.load:Qemu把app和libos的image镜像加载到内存

2.init:RustSBI完成基本硬件初始化，跳转到libos起始位置完成app运行前的初始化(建立栈空间和清零bss段)

3.run:跳转app运行，函数调用得到libos的OS服务

![alt text](image.png)
-------------------------------------------------------------------
./os/src
Rust        4 Files   119 Lines
Assembly    1 Files    11 Lines

├── bootloader(内核依赖的运行在 M 特权级的 SBI 实现，本项目中我们使用 RustSBI)
│   └── rustsbi-qemu.bin(可运行在 qemu 虚拟机上的预编译二进制版本)
├── LICENSE
├── os(我们的内核实现放在 os 目录下)
│   ├── Cargo.toml(内核实现的一些配置文件)
│   ├── Makefile
│   └── src(所有内核的源代码放在 os/src 目录下)
│       ├── console.rs(将打印字符的 SBI 接口进一步封装实现更加强大的格式化输出)
│       ├── entry.asm(设置内核执行环境的的一段汇编代码)
│       ├── lang_items.rs(需要我们提供给 Rust 编译器的一些语义项，目前包含内核 panic 时的处理逻辑)
│       ├── linker-qemu.ld(控制内核内存布局的链接脚本以使内核运行在 qemu 虚拟机上)
│       ├── main.rs(内核主函数)
│       └── sbi.rs(调用底层 SBI 实现提供的 SBI 接口)
├── README.md
└── rust-toolchain(控制整个项目的工具链版本)
-------------------------------------------------------------------
先在Linux上开发并运行一个简单的 “Hello, world” 应用程序

$cargo new os --bin
$tree os

os
├── Cargo.toml
└── src
    └── main.rs

1 directory, 2 files

$cd os
$cargo run

 Compiling os v0.1.0 (/home/shinbokuow/workspace/v3/rCore-Tutorial-v3/os)
    Finished dev [unoptimized + debuginfo] target(s) in 1.15s
     Running `target/debug/os`
Hello, world!

$ strace target/debug/os
可以查看系统调用
-------------------------------------------------------------------
$rustc --version --verbose
   rustc 1.57.0-nightly (e1e9319d9 2021-10-14)
   binary: rustc
   commit-hash: e1e9319d93aea755c444c8f8ff863b0936d7a4b6
   commit-date: 2021-10-14
   host: x86_64-unknown-linux-gnu
   release: 1.57.0-nightly
   LLVM version: 13.0.0
   Rust编译器通过 目标三元组 (Target Triplet) 来描述一个软件运行的目标平台。它一般包括 CPU、操作系统和运行时库等信息，从而控制Rust编译器可执行代码生成: 其中的 host 一项可以看出默认的目标平台是 x86_64-unknown-linux-gnu，其中 CPU 架构是 x86_64，CPU 厂商是 unknown，操作系统是 linux，运行时库是 GNU libc（封装了 Linux 系统调用，并提供 POSIX 接口为主的函数库）

   我们希望能够在另一个硬件平台上运行程序，即将 CPU 架构从 x86_64 换成 RISC-V
$rustc --print target-list | grep riscv
   通过上面命令可以看一下目前 Rust 编译器支持哪些基于 RISC-V 的目标平台
    riscv32gc-unknown-linux-gnu
    riscv32gc-unknown-linux-musl
    riscv32i-unknown-none-elf
    riscv32im-risc0-zkvm-elf
    riscv32im-unknown-none-elf
    riscv32ima-unknown-none-elf
    riscv32imac-esp-espidf
    riscv32imac-unknown-none-elf
    riscv32imac-unknown-xous-elf
    riscv32imafc-esp-espidf
    riscv32imafc-unknown-none-elf
    riscv32imc-esp-espidf
    riscv32imc-unknown-none-elf
    riscv64-linux-android
    riscv64gc-unknown-freebsd
    riscv64gc-unknown-fuchsia
    riscv64gc-unknown-hermit
    riscv64gc-unknown-linux-gnu
    riscv64gc-unknown-linux-musl
    riscv64gc-unknown-netbsd
    riscv64gc-unknown-none-elf
    riscv64gc-unknown-openbsd
    riscv64imac-unknown-none-elf
我们选择 riscv64gc-unknown-none-elf 目标平台
中的 CPU 架构是 riscv64gc ，CPU厂商是 unknown ，操作系统是 none ， elf 表示没有标准的运行时库（表明没有任何系统调用的封装支持），但可以生成 ELF 格式的执行程序

以上实现为了实现裸机环境的目的
-------------------------------------------------------------------
切换平台：
$cargo run --target riscv64gc-unknown-none-elf
    报错了因为我们已经在 rustup 工具链中安装了这个目标平台支持，因此并不是该目标平台未安装的问题
    这个问题只是单纯的表示在这个目标平台上找不到 Rust 标准库 std；所选的目标平台不存在任何操作系统支持
    这样的平台通常被我们称为 裸机平台 (bare-metal)
-------------------------------------------------------------------
最简单的 Rust 应用程序进行改造使得它能够被编译到 RV64GC 裸机平台上：
-------------------------------------------------------------------
尝试移除 println! 宏及其所在的标准库:构建运行在裸机上的操作系统，就不能再依赖标准库了
$rustup target add riscv64gc-unknown-none-elf
由于后续实验需要 rustc 编译器缺省生成RISC-V 64的目标代码，所以我们首先要给 rustc 添加一个target : riscv64gc-unknown-none-elf

$ vim os/.cargo/config 添加如下

[build]
target = "riscv64gc-unknown-none-elf"

交叉编译
-------------------------------------------------------------------
$ cargo build

如果报std未找到错，就在 main.rs 的开头加上一行 \#![no_std]

$ cargo build
继续报错是因为println宏由标准库std提供，且会使用到一个名为 write 的系统调用
现在我们的代码功能还不足以自己实现一个 println!宏；不能在核心库 core 中找到系统调用
通过//注释掉println!来先暂时跳过
-------------------------------------------------------------------
$ cargo build

又又又报错了error: `#[panic_handler]` function required, but not found
Rust编译器在编译程序时，从安全性考虑，需要有 panic! 宏的具体实现
缺少panic!宏
panic! 宏最典型的应用场景包括断言宏 assert! 失败或者对 Option::None/Result::Err 进行 unwrap 操作
-------------------------------------------------------------------
我们创建一个新的子模块 lang_items.rs 实现panic函数
// os/src/lang_items.rs

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

并通过 #[panic_handler] 属性通知编译器用panic函数来对接 panic! 宏
之后我们会从 PanicInfo 解析出错位置并打印出来，然后杀死应用程序。但目前我们什么都不做只是在原地 loop

// os/src/main.rs
#![no_std]
mod lang_items;
// ... other code

为了将该子模块添加到项目中，我们还需要在 main.rs 的 #![no_std] 的下方加上 mod lang_items
-------------------------------------------------------------------
$ cargo build
error: using `fn main` requires the standard library
移除 main 函数

(start 语义项代表了标准库 std 在执行应用程序之前需要进行的一些初始化工作)

我们在 main.rs 的开头加入设置 #![no_main] 告诉编译器我们没有一般意义上的 main 函数，并将原来的 main 函数删除
在失去了 main 函数的情况下，编译器也就不需要完成所谓的初始化工作了
-------------------------------------------------------------------
$ cargo build
至此成功编译完通过

目前的主要代码包括 main.rs 和 lang_items.rs 
// os/src/main.rs
#![no_main]
#![no_std]
mod lang_items;
// ... other code


// os/src/lang_items.rs
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
-------------------------------------------------------------------
分析被移除标准库的程序：
对于上面这个被移除标准库的应用程序，通过了Rust编译器的检查和编译，形成了二进制代码
但这个二进制代码的内容是什么，它能否在RISC-V 64计算机上正常执行呢？
为了分析这个二进制可执行程序，首先需要安装 cargo-binutils 工具集：

$ cargo install cargo-binutils
$ rustup component add llvm-tools-preview
-------------------------------------------------------------------
$ file target/riscv64gc-unknown-none-elf/debug/os
可以查看文件进行分析:
target/riscv64gc-unknown-none-elf/debug/os: ELF 64-bit LSB executable, UCB RISC-V, RVC, double-float ABI, version 1 (SYSV), statically linked, with debug_info, not stripped

$ rust-readobj -h target/riscv64gc-unknown-none-elf/debug/os
查看文件头信息:
File: target/riscv64gc-unknown-none-elf/debug/os
Format: elf64-littleriscv
Arch: riscv64
AddressSize: 64bit
LoadName: <Not found>
ElfHeader {
  Ident {
    Magic: (7F 45 4C 46)
    Class: 64-bit (0x2)
    DataEncoding: LittleEndian (0x1)
    FileVersion: 1
    OS/ABI: SystemV (0x0)
    ABIVersion: 0
    Unused: (00 00 00 00 00 00 00)
  }
  Type: Executable (0x2)
  Machine: EM_RISCV (0xF3)
  Version: 1
  Entry: 0x0
  ProgramHeaderOffset: 0x40
  SectionHeaderOffset: 0x10E0
  Flags [ (0x5)
    EF_RISCV_FLOAT_ABI_DOUBLE (0x4)
    EF_RISCV_RVC (0x1)
  ]
  HeaderSize: 64
  ProgramHeaderEntrySize: 56
  ProgramHeaderCount: 4
  SectionHeaderEntrySize: 64
  SectionHeaderCount: 12
  StringTableSectionIndex: 10
}

通过 file 工具对二进制程序 os 的分析可以看到它好像是一个合法的 RISC-V 64 可执行程序，但通过 rust-readobj 工具进一步分析，发现它的入口地址 Entry 是 0 ，从 C/C++ 等语言中得来的经验告诉我们， 0 一般表示 NULL 或空指针，因此等于 0 的入口地址看上去无法对应到任何指令。再通过 rust-objdump 工具把它反汇编，可以看到没有生成汇编代码
所以，我们可以断定，这个二进制程序虽然合法，但它是一个空程序。产生该现象的原因是：目前我们的程序（参考上面的源代码）没有进行任何有意义的工作，由于我们移除了 main 函数并将项目设置为 #![no_main] ，它甚至没有一个传统意义上的入口点（即程序首条被执行的指令所在的位置），因此 Rust 编译器会生成一个空程序

$ rust-objdump -S target/riscv64gc-unknown-none-elf/debug/os
反汇编导出汇编程序:
target/riscv64gc-unknown-none-elf/debug/os:     file format elf64-littleriscv
-------------------------------------------------------------------
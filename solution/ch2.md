------------------------------------------------------------------------------------------------------------------------------
── os
│   ├── Cargo.toml
│   ├── Makefile (修改：构建内核之前先构建应用)
│   ├── build.rs (新增：生成 link_app.S 将应用作为一个数据段链接到内核)
│   └── src
│       ├── batch.rs(新增：实现了一个简单的批处理系统)
│       ├── console.rs
│       ├── entry.asm
│       ├── lang_items.rs
│       ├── link_app.S(构建产物，由 os/build.rs 输出)
│       ├── linker.ld
│       ├── logging.rs
│       ├── main.rs(修改：主函数中需要初始化 Trap 处理并加载和执行应用)
│       ├── sbi.rs
│       ├── sync(新增：包装了RefCell，暂时不用关心)
│       │   ├── mod.rs
│       │   └── up.rs
│       ├── syscall(新增：系统调用子模块 syscall)
│       │   ├── fs.rs(包含文件 I/O 相关的 syscall)
│       │   ├── mod.rs(提供 syscall 方法根据 syscall ID 进行分发处理)
│       │   └── process.rs(包含任务处理相关的 syscall)
│       └── trap(新增：Trap 相关子模块 trap)
│           ├── context.rs(包含 Trap 上下文 TrapContext)
│           ├── mod.rs(包含 Trap 处理入口 trap_handler)
│           └── trap.S(包含 Trap 上下文保存与恢复的汇编代码)
└── user(新增：应用测例保存在 user 目录下)
   ├── Cargo.toml
   ├── Makefile
   └── src
      ├── bin(基于用户库 user_lib 开发的应用，每个应用放在一个源文件中)
      │   ├── ...
      ├── console.rs
      ├── lang_items.rs
      ├── lib.rs(用户库 user_lib)
      ├── linker.ld(应用的链接脚本)
      └── syscall.rs(包含 syscall 方法生成实际用于系统调用的汇编指令，
                     各个具体的 syscall 都是通过 syscall 来实现的)
------------------------------------------------------------------------------------------------------------------------------
$ git clone https://github.com/LearningOS/rCore-Tutorial-Code-2025S.git
$ cd rCore-Tutorial-Code-2025S
$ git checkout ch2
$ git clone https://github.com/LearningOS/rCore-Tutorial-Test-2025S.git user

ch2引入用户程序。为了将内核与应用解耦，于是将二者分成了两个仓库
user/src/bin/*.rs: 各个应用程序
user/src/*.rs: 用户库（包括入口函数、初始化函数、I/O函数和系统调用接口等）
user/src/linker.ld: 应用程序的内存布局说明

user/src/bin 里面有多个文件，其中三个是：
hello_world：在屏幕上打印一行 Hello, world!
bad_address：访问一个非法的物理地址，测试批处理系统是否会被该错误影响
power：不断在计算操作和打印字符串操作之间切换
------------------------------------------------------------------------------------------------------------------------------
# lib.rs 用户库
------------------------------------------------------------------------------------------------------------------------------
lib.rs等价于其他编程语言提供的标准库,是源程序（bin目录下）所依赖的用户库
// user/src/lib.rs
用户库的入口点 _start：

1#[no_mangle]
2#[link_section = ".text.entry"]
3pub extern "C" fn _start() -> ! {
4    clear_bss();
5    exit(main());
6}



lib.rs 中看到了另一个 main:

1#![feature(linkage)]    // 启用弱链接特性
2
3#[linkage = "weak"]
4#[no_mangle]
5fn main() -> i32 {
6    panic!("Cannot find main!");
7}

使用 Rust 宏将其标志为弱链接。这样在最后链接的时候， 虽然 lib.rs 和 bin 目录下的某个应用程序中都有 main 符号， 但由于 lib.rs 中的 main 符号是弱链接， 链接器会使用 bin 目录下的函数作为 main 。 如果在 bin 目录下找不到任何 main ，那么编译也能通过，但会在运行时报错
------------------------------------------------------------------------------------------------------------------------------
# 内存布局
------------------------------------------------------------------------------------------------------------------------------
我们使用链接脚本 user/src/linker.ld 规定用户程序的内存布局：
将程序的起始物理地址调整为 0x80400000 ，三个应用程序都会被加载到这个物理地址上运行；
将 _start 所在的 .text.entry 放在整个程序的开头 0x80400000； 批处理系统在加载应用后，跳转到 0x80400000，就进入了用户库的 _start 函数；
提供了最终生成可执行文件的 .bss 段的起始和终止地址，方便 clear_bss 函数使用。
------------------------------------------------------------------------------------------------------------------------------
# 系统调用
------------------------------------------------------------------------------------------------------------------------------
在子模块 syscall 中我们来通过 ecall 调用批处理系统提供的接口
ecall 指令会触发名为 Environment call from U-mode 的异常 ( 应用程序运行在用户态（即 U 模式）)
并 Trap 进入 S 模式执行批处理系统针对这个异常特别提供的服务程序,这个接口被称为 ABI 或者系统调用
------------------------------------------------------------------------------------------------------------------------------
/// 功能：将内存中缓冲区中的数据写入文件。
/// 参数：`fd` 表示待写入文件的文件描述符；
///      `buf` 表示内存中缓冲区的起始地址；
///      `len` 表示内存中缓冲区的长度。
/// 返回值：返回成功写入的长度。
/// syscall ID：64
fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize;

/// 功能：退出应用程序并将返回值告知批处理系统。
/// 参数：`xstate` 表示应用程序的返回值。
/// 返回值：该系统调用不应该返回。
/// syscall ID：93
fn sys_exit(xstate: usize) -> !;

按照 RISC-V 调用规范:在合适的寄存器中放置参数，然后执行ecall指令触发Trap;当Trap结束,回到U模式后,用户程序会从ecall的下一条指令继续执行，同时在合适的寄存器中读取返回值
------------------------------------------------------------------------------------------------------------------------------
RISC-V 寄存器编号从 0~31 ，表示为 x0~x31 。 其中： - x10~x17 : 对应 a0~a7 - x1 ：对应 ra
约定寄存器 a0~a6 保存系统调用的参数， a0 保存系统调用的返回值， 寄存器 a7 用来传递 syscall ID

这超出了 Rust 语言的表达能力，我们需要内嵌汇编来完成参数/返回值绑定和 ecall 指令的插入：
 1// user/src/syscall.rs
 2
 3fn syscall(id: usize, args: [usize; 3]) -> isize {
 4   let mut ret: isize;
 5   unsafe {
 6       core::arch::asm!(
 7           "ecall",
 8           inlateout("x10") args[0] => ret,   //a0
 9           in("x11") args[1],                 //a1
10           in("x12") args[2],                 //a2
11           in("x17") id                       //a7
12       );
13   }
14   ret
15}
------------------------------------------------------------------------------------------------------------------------------
将所有的系统调用都封装成 syscall 函数，可以看到它支持传入 syscall ID 和 3 个参数
使用 Rust 提供的 asm! 宏在代码中内嵌汇编;
因为Rust 编译器无法判定汇编代码的安全性，所以我们需要将其包裹在 unsafe 块中
------------------------------------------------------------------------------------------------------------------------------
于是 sys_write 和 sys_exit 只需将 syscall 进行包装：

1// user/src/syscall.rs
 2
 3const SYSCALL_WRITE: usize = 64;
 4const SYSCALL_EXIT: usize = 93;
 5
 6pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
 7    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
 8}
 9
10pub fn sys_exit(xstate: i32) -> isize {
11    syscall(SYSCALL_EXIT, [xstate as usize, 0, 0])
12}

我们将上述两个系统调用在用户库 user_lib 中进一步封装，像标准库一样：

1// user/src/lib.rs
2use syscall::*;
3
4pub fn write(fd: usize, buf: &[u8]) -> isize { sys_write(fd, buf) }
5pub fn exit(exit_code: i32) -> isize { sys_exit(exit_code) }

在 console 子模块中，借助 write，我们为应用程序实现了 println! 宏。 传入到 write 的 fd 参数设置为 1，代表标准输出 STDOUT，暂时不用考虑其他的 fd 选取情况

pub const STDOUT: usize = 1;

impl ConsoleBuffer {
    fn flush(&mut self) -> isize {
        let s: &[u8] = self.0.make_contiguous();
        let ret = write(STDOUT, s);
        self.0.clear();
        ret
    }
}
------------------------------------------------------------------------------------------------------------------------------
# 编译生成应用程序二进制码
------------------------------------------------------------------------------------------------------------------------------
简要介绍一下应用程序的构建，在 user 目录下 make build：
对于 src/bin 下的每个应用程序， 在 target/riscv64gc-unknown-none-elf/release 目录下生成一个同名的 ELF 可执行文件；
使用 objcopy 二进制工具删除所有 ELF header 和符号，得到 .bin 后缀的纯二进制镜像文件。 它们将被链接进内核，并由内核在合适的时机加载到内存。
------------------------------------------------------------------------------------------------------------------------------
# 将应用程序链接到内核
------------------------------------------------------------------------------------------------------------------------------
把应用程序的二进制镜像文件作为数据段链接到内核里， 内核需要知道应用程序的数量和它们的位置
// os/src/main.rs
core::arch::global_asm!(include_str!("link_app.S"));
这里引入了汇编代码，是make build 构建操作系统时自动生成由脚本 os/build.rs 控制生成的
 1# os/src/link_app.S
 2
 3    .align 3
 4    .section .data
 5    .global _num_app
 6_num_app:
 7    .quad 3
 8    .quad app_0_start
 9    .quad app_1_start
10    .quad app_2_start
11    .quad app_2_end
12
13    .section .data
14    .global app_0_start
15    .global app_0_end
16app_0_start:
17    .incbin "../user/target/riscv64gc-unknown-none-elf/release/hello_world.bin"
18app_0_end:
19
20    .section .data
21    .global app_1_start
22    .global app_1_end
23app_1_start:
24    .incbin "../user/target/riscv64gc-unknown-none-elf/release/bad_address.bin"
25app_1_end:
26
27    .section .data
28    .global app_2_start
29    .global app_2_end
30app_2_start:
31    .incbin "../user/target/riscv64gc-unknown-none-elf/release/power.bin"
32app_2_end:

大致内容如上（只举例了三个app）
第 13 行开始的三个数据段分别插入了三个应用程序的二进制镜像， 并且各自有一对全局符号 app_*_start, app_*_end 指示它们的开始和结束位置。 
而第 3 行开始的另一个数据段相当于一个 64 位整数数组。 数组中的第一个元素表示应用程序的数量，后面则按照顺序放置每个应用程序的起始地址， 最后一个元素放置最后一个应用程序的结束位置。这样数组中相邻两个元素记录了每个应用程序的始末位置， 这个数组所在的位置由全局符号 _num_app 所指示
------------------------------------------------------------------------------------------------------------------------------
# 找到并加载应用程序二进制码
------------------------------------------------------------------------------------------------------------------------------
// os/batch.rs
应用管理器 AppManager:

struct AppManager {
    num_app: usize,
    current_app: usize,
    app_start: [usize; MAX_APP_NUM + 1],
}

初始化其全局实例:
找到 link_app.S 中提供的符号 _num_app ，并从这里开始解析出应用数量以及各个应用的开头地址
用容器 UPSafeCell 包裹 AppManager 是为了防止全局对象 APP_MANAGER 被重复获取
(UPSafeCell 实现在 sync 模块中，调用 exclusive_access 方法能获取其内部对象的可变引用， 如果程序运行中同时存在多个这样的引用，会触发 already borrowed: BorrowMutError)

lazy_static! {
    static ref APP_MANAGER: UPSafeCell<AppManager> = unsafe {
        UPSafeCell::new({
            extern "C" {
                fn _num_app();
            }
            let num_app_ptr = _num_app as usize as *const usize;
            let num_app = num_app_ptr.read_volatile();
            let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
            let app_start_raw: &[usize] =
                core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1);
            app_start[..=num_app].copy_from_slice(app_start_raw);
            AppManager {
                num_app,
                current_app: 0,
                app_start,
            }
        })
    };
}

lazy_static!宏(​​延迟初始化​​的静态变量) 提供了全局变量的运行时初始化功能。一般情况下，全局变量必须在编译期设置初始值
但是有些全局变量的初始化依赖于运行期间才能得到的数据,如这里我们借助lazy_static! 声明了一个 AppManager 结构的名为 APP_MANAGER 的全局实例
只有在它第一次被使用到的时候才会进行实际的初始化工作
调用 print_app_info 的时第一次用到了全局变量 APP_MANAGER ，它在这时完成初始化
------------------------------------------------------------------------------------------------------------------------------
AppManager 的方法中， print_app_info/get_current_app/move_to_next_app 都相当简单直接，需要说明的是 load_app：

 1unsafe fn load_app(&self, app_id: usize) {

 2    if app_id >= self.num_app {
 3        panic!("All applications completed!");
 4    }
 5    info!("[kernel] Loading app_{}", app_id);

 6    // clear icache
 7    core::arch::asm!("fence.i");

 8    // clear app area
 9    core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);

10    let app_src = core::slice::from_raw_parts(
11        self.app_start[app_id] as *const u8,
12        self.app_start[app_id + 1] - self.app_start[app_id],
13    );
14    let app_dst = core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
15    app_dst.copy_from_slice(app_src);
16}

这个方法负责将参数 app_id 对应的应用程序的二进制镜像加载到物理内存以 0x80400000 起始的位置，这个位置是批处理操作系统和应用程序之间约定的常数地址。
我们将从这里开始的一块内存清空，然后找到待加载应用二进制镜像的位置，并将它复制到正确的位置。

清空内存前，我们插入了一条奇怪的汇编指令 fence.i ，它是用来清理 i-cache 的。 我们知道，缓存又分成数据缓存 (d-cache) 和 指令缓存(i-cache) 两部分，分别在 CPU 访存和取指的时候使用。 通常情况下， CPU 会认为程序的代码段不会发生变化，因此 i-cache 是一种只读缓存！ 但在这里，我们将会修改被 CPU 取指的内存区域，使得 i-cache 中含有与内存不一致的内容，必须用 fence.i 指令手动清空 i-cache ，让里面所有的内容全部失效， 才能够保证程序执行正确性。
------------------------------------------------------------------------------------------------------------------------------
batch子模块对外暴露出如下接口：
init ：调用 print_app_info 的时第一次用到了全局变量 APP_MANAGER ，它在这时完成初始化
run_next_app ：批处理操作系统的核心操作，即加载并运行下一个应用程序。 批处理操作系统完成初始化，或者应用程序运行结束/出错后会调用该函数。
------------------------------------------------------------------------------------------------------------------------------

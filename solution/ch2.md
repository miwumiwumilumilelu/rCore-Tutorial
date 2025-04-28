------------------------------------------------------------------------------------------------------------------------------
```
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
```        
------------------------------------------------------------------------------------------------------------------------------
```
$ git clone https://github.com/LearningOS/rCore-Tutorial-Code-2025S.git
$ cd rCore-Tutorial-Code-2025S
$ git checkout ch2
$ git clone https://github.com/LearningOS/rCore-Tutorial-Test-2025S.git user
```

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
```
1#[no_mangle]
2#[link_section = ".text.entry"]
3pub extern "C" fn _start() -> ! {
4    clear_bss();
5    exit(main());
6}
```


lib.rs 中看到了另一个 main:
```
1#![feature(linkage)]    // 启用弱链接特性
2
3#[linkage = "weak"]
4#[no_mangle]
5fn main() -> i32 {
6    panic!("Cannot find main!");
7}
```
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
```
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
```
按照 RISC-V 调用规范:在合适的寄存器中放置参数，然后执行ecall指令触发Trap;当Trap结束,回到U模式后,用户程序会从ecall的下一条指令继续执行，同时在合适的寄存器中读取返回值

------------------------------------------------------------------------------------------------------------------------------
RISC-V 寄存器编号从 0~31 ，表示为 x0~x31 。 其中： - x10~x17 : 对应 a0~a7 - x1 ：对应 ra
约定寄存器 a0~a6 保存系统调用的参数， a0 保存系统调用的返回值， 寄存器 a7 用来传递 syscall ID

这超出了 Rust 语言的表达能力，我们需要内嵌汇编来完成参数/返回值绑定和 ecall 指令的插入：
```
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
```
------------------------------------------------------------------------------------------------------------------------------
将所有的系统调用都封装成 syscall 函数，可以看到它支持传入 syscall ID 和 3 个参数
使用 Rust 提供的 asm! 宏在代码中内嵌汇编;
因为Rust 编译器无法判定汇编代码的安全性，所以我们需要将其包裹在 unsafe 块中

------------------------------------------------------------------------------------------------------------------------------
于是 sys_write 和 sys_exit 只需将 syscall 进行包装：
```
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
```
我们将上述两个系统调用在用户库 user_lib 中进一步封装，像标准库一样：
```
1// user/src/lib.rs
2use syscall::*;
3
4pub fn write(fd: usize, buf: &[u8]) -> isize { sys_write(fd, buf) }
5pub fn exit(exit_code: i32) -> isize { sys_exit(exit_code) }
```
在 console 子模块中，借助 write，我们为应用程序实现了 println! 宏。 传入到 write 的 fd 参数设置为 1，代表标准输出 STDOUT，暂时不用考虑其他的 fd 选取情况
```
pub const STDOUT: usize = 1;

impl ConsoleBuffer {
    fn flush(&mut self) -> isize {
        let s: &[u8] = self.0.make_contiguous();
        let ret = write(STDOUT, s);
        self.0.clear();
        ret
    }
}
```
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
```
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
```
大致内容如上（只举例了三个app）
第 13 行开始的三个数据段分别插入了三个应用程序的二进制镜像， 并且各自有一对全局符号 app_*_start, app_*_end 指示它们的开始和结束位置。 
而第 3 行开始的另一个数据段相当于一个 64 位整数数组。 数组中的第一个元素表示应用程序的数量，后面则按照顺序放置每个应用程序的起始地址， 最后一个元素放置最后一个应用程序的结束位置。这样数组中相邻两个元素记录了每个应用程序的始末位置， 这个数组所在的位置由全局符号 _num_app 所指示

------------------------------------------------------------------------------------------------------------------------------
# 找到并加载应用程序二进制码
------------------------------------------------------------------------------------------------------------------------------
```
// os/batch.rs
应用管理器 AppManager:

struct AppManager {
    num_app: usize,
    current_app: usize,
    app_start: [usize; MAX_APP_NUM + 1],
}
```
初始化其全局实例:
找到 link_app.S 中提供的符号 _num_app ，并从这里开始解析出应用数量以及各个应用的开头地址
用容器 UPSafeCell 包裹 AppManager 是为了防止全局对象 APP_MANAGER 被重复获取
(UPSafeCell 实现在 sync 模块中，调用 exclusive_access 方法能获取其内部对象的可变引用， 如果程序运行中同时存在多个这样的引用，会触发 already borrowed: BorrowMutError)
```
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
```
lazy_static!宏(​​延迟初始化​​的静态变量) 提供了全局变量的运行时初始化功能。一般情况下，全局变量必须在编译期设置初始值
但是有些全局变量的初始化依赖于运行期间才能得到的数据,如这里我们借助lazy_static! 声明了一个 AppManager 结构的名为 APP_MANAGER 的全局实例
只有在它第一次被使用到的时候才会进行实际的初始化工作
调用 print_app_info 的时第一次用到了全局变量 APP_MANAGER ，它在这时完成初始化

------------------------------------------------------------------------------------------------------------------------------
AppManager 的方法中， print_app_info/get_current_app/move_to_next_app 都相当简单直接，需要说明的是 load_app：
```
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
```
这个方法负责将参数 app_id 对应的应用程序的二进制镜像加载到物理内存以 0x80400000 起始的位置，这个位置是批处理操作系统和应用程序之间约定的常数地址。
我们将从这里开始的一块内存清空，然后找到待加载应用二进制镜像的位置，并将它复制到正确的位置。

清空内存前，我们插入了一条奇怪的汇编指令 fence.i ，它是用来清理 i-cache 的。 我们知道，缓存又分成数据缓存 (d-cache) 和 指令缓存(i-cache) 两部分，分别在 CPU 访存和取指的时候使用。 通常情况下， CPU 会认为程序的代码段不会发生变化，因此 i-cache 是一种只读缓存！ 但在这里，我们将会修改被 CPU 取指的内存区域，使得 i-cache 中含有与内存不一致的内容，必须用 fence.i 指令手动清空 i-cache ，让里面所有的内容全部失效， 才能够保证程序执行正确性。

------------------------------------------------------------------------------------------------------------------------------
batch子模块对外暴露出如下接口：
init ：调用 print_app_info 的时第一次用到了全局变量 APP_MANAGER ，它在这时完成初始化
run_next_app ：批处理操作系统的核心操作，即加载并运行下一个应用程序。 批处理操作系统完成初始化，或者应用程序运行结束/出错后会调用该函数。

------------------------------------------------------------------------------------------------------------------------------
# RISC-V特权级切换
------------------------------------------------------------------------------------------------------------------------------
特权级切换相关的控制状态寄存器:

sstatus
SPP 等字段给出 Trap 发生之前 CPU 处在哪个特权级（S/U）等信息

sepc
当 Trap 是一个异常的时候，记录 Trap 发生之前执行的最后一条指令的地址

scause
描述 Trap 的原因

stval
给出 Trap 附加信息

stvec
控制 Trap 处理代码的入口地址

------------------------------------------------------------------------------------------------------------------------------
特权级切换的具体过程一部分由硬件直接完成，另一部分则需要由操作系统来实现:

特权级切换的硬件控制机制:

当 CPU 执行完一条指令并准备从用户特权级 陷入（ Trap ）到 S 特权级的时候，硬件会自动完成如下这些事情：
sstatus 的 SPP 字段会被修改为 CPU 当前的特权级（U/S）。
sepc 会被修改为 Trap 处理完成后默认会执行的下一条指令的地址。
scause/stval 分别会被修改成这次 Trap 的原因以及相关的附加信息。
CPU 会跳转到 stvec 所设置的 Trap 处理入口地址，并将当前特权级设置为 S ，然后从Trap 处理入口地址处开始执行。

------------------------------------------------------------------------------------------------------------------------------
当 CPU 完成 Trap 处理准备返回的时候，需要通过一条 S 特权级的特权指令 sret:

sret实现功能:
CPU 会将当前的特权级按照 sstatus 的 SPP 字段设置为 U 或者 S ；
CPU 会跳转到 sepc 寄存器指向的那条指令，然后继续执行。

------------------------------------------------------------------------------------------------------------------------------
# 用户栈与内核栈
------------------------------------------------------------------------------------------------------------------------------
在 Trap 触发的一瞬间， CPU 会切换到 S 特权级并跳转到 stvec 所指示的位置。
但是在正式进入 S 特权级的 Trap 处理之前，我们必须保存原控制流的寄存器状态，这一般通过栈来完成
但我们需要用专门为操作系统准备的内核栈，而不是应用程序运行时用到的用户栈
```
 1// os/src/batch.rs
 2
 3#[repr(align(4096))]
 4struct KernelStack {
 5  data: [u8; KERNEL_STACK_SIZE],
 6}
 7
 8#[repr(align(4096))]
 9struct UserStack {
10  data: [u8; USER_STACK_SIZE],
11}
12
13static KERNEL_STACK: KernelStack = KernelStack {
14  data: [0; KERNEL_STACK_SIZE],
15};
16static USER_STACK: UserStack = UserStack {
17  data: [0; USER_STACK_SIZE],
18};
```
KernelStack 和 UserStack分别表示用户栈和内核栈
两个栈以全局变量的形式实例化在批处理操作系统的 .bss 段中

------------------------------------------------------------------------------------------------------------------------------
由于在 RISC-V 中栈是向下增长的， 我们只需返回包裹的数组的结尾地址，以用户栈类型 UserStack 为例:
```
1impl UserStack {
2    fn get_sp(&self) -> usize {
3        self.data.as_ptr() as usize + USER_STACK_SIZE
4    }
5}
```
实现了 get_sp 方法来获取栈顶地址
换栈是非常简单的，只需将 sp 寄存器的值修改为 get_sp 的返回值即可

------------------------------------------------------------------------------------------------------------------------------
接下来是 Trap 上下文，即在 Trap 发生时需要保存的物理资源内容，定义如下:
```
1// os/src/trap/context.rs
2
3#[repr(C)]
4pub struct TrapContext {
5    pub x: [usize; 32],
6    pub sstatus: Sstatus,
7    pub sepc: usize,
8}
```
包含所有的通用寄存器 x0~x31 ，还有 sstatus 和 sepc
scause/stval 的情况是：它总是在 Trap 处理的第一时间就被使用或者是在其他地方保存下来了，因此它没有被修改并造成不良影响的风险。 而对于 sstatus/sepc 而言，它们会在 Trap 处理的全程有意义
而且确实会出现 Trap 嵌套的情况使得它们的值被覆盖掉,所以我们需要将它们也一起保存下来，并在 sret 之前恢复原样

------------------------------------------------------------------------------------------------------------------------------
# Trap 管理
------------------------------------------------------------------------------------------------------------------------------
## Trap 上下文的保存与恢复
------------------------------------------------------------------------------------------------------------------------------
在批处理操作系统初始化时，我们需要修改 stvec 寄存器来指向正确的 Trap 处理入口点
```
 1// os/src/trap/mod.rs
 2
 3core::arch::global_asm!(include_str!("trap.S"));
 4
 5pub fn init() {
 6    extern "C" { fn __alltraps(); }
 7    unsafe {
 8        stvec::write(__alltraps as usize, TrapMode::Direct);
 9    }
10}
```
引入了一个外部符号 __alltraps ，并将 stvec 设置为 Direct 模式指向它的地址。我们在 os/src/trap/trap.S 中实现 Trap 上下文保存/恢复的汇编代码，分别用外部符号 __alltraps 和 __restore 标记为函数，并通过 global_asm! 宏将 trap.S 这段汇编代码插入进来

Trap 处理的总体流程如下：首先通过 __alltraps 将 Trap 上下文保存在内核栈上，然后跳转到使用 Rust 编写的 trap_handler 函数 完成 Trap 分发及处理。当 trap_handler 返回之后，使用 __restore 从保存在内核栈上的 Trap 上下文恢复寄存器。最后通过一条 sret 指令回到应用程序执行

------------------------------------------------------------------------------------------------------------------------------
__alltraps:
```
 1# os/src/trap/trap.S
 2
 3.macro SAVE_GP n                                                  //宏定义 SAVE_GP
 4    sd x\n, \n*8(sp)                                              //每个寄存器占8字节，通过宏展开，将寄存器按编号存入栈中对应位置
 5.endm
 6
 7.align 2
 8__alltraps:
 9    csrrw sp, sscratch, sp                                        //交换 sp 和 sscratch 的值(sp->sscratch->sp)
10    # now sp->kernel stack, sscratch->user stack 
11    # allocate a TrapContext on kernel stack                      //预留空间用于保存 TrapContext 结构体，包含所有需要保存的寄存器和状态信息
12    addi sp, sp, -34*8                                            //在内核栈上分配 34 * 8 字节（272字节）的空间;
13    # save general-purpose registers                              //依次保存 x1（返回地址）、x3（全局指针）和 x5~x31 到栈中                 
14    sd x1, 1*8(sp)                                                
15    # skip sp(x2), we will save it later                          //跳过 x2（sp）​​：此时 sp 已指向内核栈，用户栈指针后续单独保存
16    sd x3, 3*8(sp)
17    # skip tp(x4), application does not use it                    //​跳过 x4（tp）​​：线程指针通常由内核管理，用户程序无需修改
18    # save x5~x31
19    .set n, 5
20    .rept 27
21        SAVE_GP %n
22        .set n, n+1
23    .endr
24    # we can use t0/t1/t2 freely, because they were saved on kernel stack
25    csrr t0, sstatus
26    csrr t1, sepc
27    sd t0, 32*8(sp)
28    sd t1, 33*8(sp)
29    # read user stack from sscratch and save it on the kernel stack
30    csrr t2, sscratch
31    sd t2, 2*8(sp)
32    # set input argument of trap_handler(cx: &mut TrapContext)
33    mv a0, sp
34    call trap_handler
```
------------------------------------------------------------------------------------------------------------------------------
__restore:
```
 1.macro LOAD_GP n                                                   //宏定义 LOAD_GP​,与SAVE_GP对称
 2    ld x\n, \n*8(sp)
 3.endm
 4
 5__restore:
 6    # case1: start running app by __restore
 7    # case2: back to U after handling trap
 8    mv sp, a0                                                     //将参数a0（指向内核栈上的TrapContext）赋给sp   
 9    # now sp->kernel stack(after allocated), sscratch->user stack
10    # restore sstatus/sepc
11    ld t0, 32*8(sp)                                               //加载 sstatus
12    ld t1, 33*8(sp)                                               //加载 sepc
13    ld t2, 2*8(sp)                                                //加载用户栈指针（原x2）
14    csrw sstatus, t0                                              //恢复处理器状态
15    csrw sepc, t1                                                 //设置返回地址
16    csrw sscratch, t2                                             //保存用户栈到 sscratch
17    # restore general-purpuse registers except sp/tp
18    ld x1, 1*8(sp)                                                //恢复返回地址（ra）
19    ld x3, 3*8(sp)                                                //恢复全局指针（gp）
20    .set n, 5                                 
21    .rept 27                                                      //恢复 x5~x31
22        LOAD_GP %n
23        .set n, n+1
24    .endr
25    # release TrapContext on kernel stack
26    addi sp, sp, 34*8                                             //释放 TrapContext 空间
27    # now sp->kernel stack, sscratch->user stack
28    csrrw sp, sscratch, sp                                        //切换回用户栈
29    sret
```
顺序性​​：先恢复sstatus和sepc，确保sret能正确执行
​​用户栈管理​​：用户栈指针从保存的x2位置加载到sscratch，而非直接赋给sp，避免过早切换栈导致后续加载错误
切换​：通过csrrw交换sp与sscratch，此时：
sp 指向用户栈，后续指令在用户态执行
sscratch 保存内核栈指针，为下次陷阱做准备

------------------------------------------------------------------------------------------------------------------------------
## Trap 分发与处理
------------------------------------------------------------------------------------------------------------------------------
trap_handler 函数：
```
 1// os/src/trap/mod.rs
 2
 3#[no_mangle]
 4pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    //读取trap原因和附加信息
 5    let scause = scause::read();
 6    let stval = stval::read();
 7    match scause.cause() {
 8        Trap::Exception(Exception::UserEnvCall) => {
    //ecall指令长度为4字节，sepc +=4确保返回到下一条指令,避免死循环
 9            cx.sepc += 4;
    //a7为系统调用ID,三个参数，返回值a0
10            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
11        }
12        Trap::Exception(Exception::StoreFault) |
13        Trap::Exception(Exception::StorePageFault) => {
14            println!("[kernel] PageFault in application, core dumped.");
    //尝试非法内存写入,终止当前应用，启动下一个
15            run_next_app();
16        }
17        Trap::Exception(Exception::IllegalInstruction) => {
18            println!("[kernel] IllegalInstruction in application, core dumped.");
    //执行了未定义的指令,终止当前应用，启动下一个
19            run_next_app();
20        }
21        _ => {
22            panic!("Unsupported trap {:?}, stval = {:#x}!", scause.cause(), stval);
23        }
24    }
25    cx
26}
```
------------------------------------------------------------------------------------------------------------------------------
# 执行应用程序
------------------------------------------------------------------------------------------------------------------------------
当批处理操作系统初始化完成，或者是某个应用程序运行结束或出错的时候，我们要调用 run_next_app 函数切换到下一个应用程序
此时 CPU 运行在 S 特权级，而它希望能够切换到 U 特权级。在 RISC-V 架构中，唯一一种能够使得 CPU 特权级下降的方法就是通过 Trap 返回系列指令，比如 sret
事实上，在运行应用程序之前要完成如下这些工作：

跳转到应用程序入口点 0x80400000
将使用的栈切换到用户栈
在 __alltraps 时我们要求 sscratch 指向内核栈，这个也需要在此时完成
从 S 特权级切换到 U 特权级

------------------------------------------------------------------------------------------------------------------------------
让寄存器到达启动应用程序所需要的上下文状态:

------------------------------------------------------------------------------------------------------------------------------
在内核栈上压入一个为启动应用程序而特殊构造的 Trap 上下文
```
1// os/src/trap/context.rs
 2
 3impl TrapContext {
 4    pub fn set_sp(&mut self, sp: usize) { self.x[2] = sp; }
 5    pub fn app_init_context(entry: usize, sp: usize) -> Self {
 6        let mut sstatus = sstatus::read();
 7        sstatus.set_spp(SPP::User);
 8        let mut cx = Self {
 9            x: [0; 32],
10            sstatus,
11            sepc: entry,
12        };
13        cx.set_sp(sp);
14        cx
15    }
16}
```
为 TrapContext 实现 app_init_context 方法，修改其中的 sepc 寄存器为应用程序入口点 entry， sp 寄存器为我们设定的 一个栈指针，并将 sstatus 寄存器的 SPP 字段设置为 User

------------------------------------------------------------------------------------------------------------------------------
复用 __restore
```
 1// os/src/batch.rs
 2
 3pub fn run_next_app() -> ! {
 4    let mut app_manager = APP_MANAGER.exclusive_access();
 5    let current_app = app_manager.get_current_app();
 6    unsafe {
 7        app_manager.load_app(current_app);
 8    }
 9    app_manager.move_to_next_app();
10    drop(app_manager);
11    // before this we have to drop local variables related to resources manually
12    // and release the resources
13    extern "C" {
14        fn __restore(cx_addr: usize);
15    }
16    unsafe {
17        __restore(KERNEL_STACK.push_context(TrapContext::app_init_context(
18            APP_BASE_ADDRESS,
19            USER_STACK.get_sp(),
20        )) as *const _ as usize);
21    }
22    panic!("Unreachable in batch::run_current_app!");
23}
```
__restore 所做的事情是在内核栈上压入一个 Trap 上下文，其 sepc 是应用程序入口地址 0x80400000 ，其 sp 寄存器指向用户栈，其 sstatus 的 SPP 字段被设置为 User 
push_context 的返回值是内核栈压入 Trap 上下文之后的栈顶，它会被作为 __restore 的参数（ 回看 __restore 代码 ，这时我们可以理解为何 __restore 函数的起始部分会完成 
sp<-a0），这使得在 __restore 函数中 sp 仍然可以指向内核栈的栈顶。这之后，就和执行一次普通的 __restore 函数调用一样了。

------------------------------------------------------------------------------------------------------------------------------

# 编程作业
-------------------------------------------------------------------------
实现easy-fs:

本节要求实现三个系统调用 sys_linkat、sys_unlinkat、sys_stat 

核心思想是将磁盘块划分为不同的功能区域，通过元数据管理文件和目录
```
磁盘布局顺序​​:

*​​超级块（Super Block）​​
    ​​位置​​：第 ​​0​​ 号磁盘块。
    ​​作用​​：存储文件系统的全局元数据，包括：
    魔数（Magic Number，标识文件系统类型）。
    总块数、Inode 位图区域长度、数据块位图区域长度、Inode 区域长度。
    数据区域的起始块号。
    ​​合法性检查​​：通过魔数验证文件系统格式是否正确。
​*​索引节点位图（Inode Bitmap）​​
    ​​位置​​：紧接超级块的若干块。
    ​​作用​​：用位图（bitmap）标记哪些 Inode 已被分配。
    ​​细节​​：每个位对应一个 Inode，1 表示已占用，0 表示空闲。
    ​​示例​​：若块大小为 4KB，一个块可管理 4096 * 8 = 32768 个 Inode。
​​*索引节点区域（Inode Area）​​
    ​​位置​​：紧接索引节点位图的若干块。
    ​​作用​​：存储所有 Inode 结构体，每个 Inode 对应文件/目录的元数据。
    ​​Inode 结构​​：
    文件大小、权限、时间戳。
    直接指针（指向数据块）、间接指针（指向索引块）。
    ​​存储密度​​：若每个 Inode 占 256 字节，一个 4KB 块可存 4096 / 256 = 16 个 Inode。
*​​数据块位图（Data Block Bitmap）​​
    ​​位置​​：紧接索引节点区域的若干块。
    ​​作用​​：用位图标记哪些数据块已被分配。
    ​​细节​​：每个位对应一个数据块，1 表示已占用，0 表示空闲。
    ​​示例​​：若有 10,000 个数据块，需 10,000 / 8 / 4096 ≈ 0.3 块（向上取整为 1 块）。
​*​数据块区域（Data Blocks）​​
    ​​位置​​：剩余的所有块。
    ​​作用​​：存储实际文件内容和目录结构。
    ​​分配逻辑​​：
    小文件：通过 Inode 的直接指针快速访问。
    大文件：通过间接指针（指向索引块）扩展存储
```

```
如何调试 easy-fs

如果你在第一章练习题中已经借助 log crate 实现了日志功能，那么你可以直接在 easy-fs 中引入 log crate，通过 log::info!/debug! 等宏即可进行调试并在内核中看到日志输出。具体来说，在 easy-fs 中的修改是：在 easy-fs/Cargo.toml 的依赖中加入一行 log = "0.4.0"，然后在 easy-fs/src/lib.rs 中加入一行 extern crate log 。

你也可以完全在用户态进行调试。仿照 easy-fs-fuse 建立一个在当前操作系统中运行的应用程序，将测试逻辑写在 main 函数中。这个时候就可以将它引用的 easy-fs 的 no_std 去掉并使用 println! 进行调试
```

ch6中多了easy-fs功能模块：

os和easy-fs-fuse的cagro.toml中可以看到引入了easy-fs为dependency，即相当于库函数

os/Makefile中:
```
fs-img: $(APPS)
	@make -C ../user build TEST=$(TEST) CHAPTER=$(CHAPTER) BASE=$(BASE)
	@rm -f $(FS_IMG)
	@cd ../easy-fs-fuse && cargo run --release -- -s ../user/build/app/ -t ../user/target/riscv64gc-unknown-none-elf/release/
```
进入 easy-fs-fuse 目录,使用 cargo run --release 以发布模式运行该工具

-s ../user/build/app/：指定包含已编译用户程序的源目录

-t ../user/target/.../release/：指定目标目录（存放最终镜像，通常是 RISC-V 架构的可执行文件）

即 将用户程序最终写入指定目录

-------------------------------------------------------------------------
## sys_linkat

syscall ID: 37

功能：创建一个文件的一个硬链接， linkat标准接口 。

Ｃ接口： int linkat(int olddirfd, char* oldpath, int newdirfd, char* newpath, unsigned int flags)

Rust 接口： fn linkat(olddirfd: i32, oldpath: *const u8, newdirfd: i32, newpath: *const u8, flags: u32) -> i32

参数：
olddirfd，newdirfd: 仅为了兼容性考虑，本次实验中始终为 AT_FDCWD (-100)，可以忽略。

flags: 仅为了兼容性考虑，本次实验中始终为 0，可以忽略。

oldpath：原有文件路径

newpath: 新的链接文件路径。

说明：
为了方便，不考虑新文件路径已经存在的情况（属于未定义行为），除非链接同名文件。

返回值：如果出现了错误则返回 -1，否则返回 0。

可能的错误
链接同名文件。

-------------------------------------------------------------------------

-------------------------------------------------------------------------
## sys_unlinkat

syscall ID: 35

功能：取消一个文件路径到文件的链接, unlinkat标准接口 。

Ｃ接口： int unlinkat(int dirfd, char* path, unsigned int flags)

Rust 接口： fn unlinkat(dirfd: i32, path: *const u8, flags: u32) -> i32

参数：
dirfd: 仅为了兼容性考虑，本次实验中始终为 AT_FDCWD (-100)，可以忽略。

flags: 仅为了兼容性考虑，本次实验中始终为 0，可以忽略。

path：文件路径。

说明：
注意考虑使用 unlink 彻底删除文件的情况，此时需要回收inode以及它对应的数据块。

返回值：如果出现了错误则返回 -1，否则返回 0。

可能的错误
文件不存在。

-------------------------------------------------------------------------
-------------------------------------------------------------------------
## sys_stat

syscall ID: 80

功能：获取文件状态。

Ｃ接口： int fstat(int fd, struct Stat* st)

Rust 接口： fn fstat(fd: i32, st: *mut Stat) -> i32

参数：
fd: 文件描述符

st: 文件状态结构体
```
#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// 文件所在磁盘驱动器号，该实验中写死为 0 即可
    pub dev: u64,
    /// inode 文件所在 inode 编号
    pub ino: u64,
    /// 文件类型
    pub mode: StatMode,
    /// 硬链接数量，初始为1
    pub nlink: u32,
    /// 无需考虑，为了兼容性设计
    pad: [u64; 7],
}

/// StatMode 定义：
bitflags! {
    pub struct StatMode: u32 {
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}
```
-------------------------------------------------------------------------
-------------------------------------------------------------------------

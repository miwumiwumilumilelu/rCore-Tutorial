# 编程作业
-------------------------------------------------------------------------
## sys_get_time
-------------------------------------------------------------------------
获得一个TimeVal：

--------------------------------------
1.取得当前硬件计时器值并转化为微秒数，然后拆分成TimeVal格式
let us = get_time_us();
let sec = us / 1_000_000;
let usec = us % 1_000_000;

--------------------------------------
查看 get_time_us 函数
#[allow(dead_code)]
pub fn get_time_us() -> usize {
    time::read() * MICRO_PER_SEC / CLOCK_FREQ
}
time::read()读取硬件计时器的​原始计数值，即时钟周期数
将​时钟周期数​​转换为​微秒数：
微秒数 = (时钟周期数 × 1_000_000) / 时钟频率

--------------------------------------
2.获取当前任务的用户态页表基地址
current_user_token()

--------------------------------------
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

fn get_current_token(&self) -> usize {
    let inner = self.inner.exclusive_access();
    inner.tasks[inner.current_task].get_user_token()
}

pub fn get_user_token(&self) -> usize {
    self.memory_set.token()
}

memory_set 是 TaskControlBlock 的一个字段，其是一个与任务的地址空间相关的结构；
token() 方法返回的是与该地址空间相关的用户态页表基地址

--------------------------------------
3.现在有了具体的任务用户态基地址，需要把1的结果加到任务状态中,需要转化成内核可访问的地址
let ts = user_ptr_to_kernel_ref(current_user_token(), _ts);

-------------------------------------------------------------------------
在页表管理文件中编写新函数,使其实现3操作,对应两个元素，一个是usize，一个是T,返回T
完成用户态虚拟地址到内核态物理地址的安全翻译
是关键一步,手动实现翻译
// os/src/mm/page_table.rs

//虚拟地址->虚拟页号->物理页号->物理地址
pub fn user_ptr_to_kernel_ref<T>(token: usize, ptr: *mut T) -> &'static mut T {
    //根据 token 创建一个 PageTable 实例，用于操作用户态的页表
    let page_table = PageTable::from_token(token);
    //将用户态指针 ptr 转换为虚拟地址 VirtAddr 类型
    let v = VirtAddr::from(ptr as usize);
    //获取偏移量
    let offset = v.page_offset();
    //将虚拟地址转换为虚拟页号
    let vpn = v.floor();
    //将虚拟页号翻译成对应页表项并返回
    let mut p: PhysAddr = page_table.translate(vpn).unwrap().ppn().into();
    //将页内偏移量 offset 加到物理地址 p 上，得到完整的物理地址
    p.0 += offset;
    //将物理地址 p 转换为对应的内核态可变引用
    p.get_mut()
}

-------------------------------------------------------------------------
对于 Sv39 虚拟内存模式，satp 的结构如下：
43:0 PPN 根页表的物理页号
62:44 ASID 地址空间标识符，用于区分不同的地址空间
63 	MODE 地址转换模式（Sv39 模式为 8）
因此satp左移44位-1是为了获得root_ppn
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

pub fn from_token(satp: usize) -> Self {
    Self {
        root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
        frames: Vec::new(),
    }
}



Sv39 是 RISC-V 的一种虚拟内存模式，支持 39 位虚拟地址空间
因此 (1 << VA_WIDTH_SV39) - 1 是取39位有效位
impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VA_WIDTH_SV39) - 1))
    }
}



let page_off = va.page_offset();
pub fn page_offset(&self) -> usize {
    self.0 & (PAGE_SIZE - 1)    // 0xfff
}
PAGE_SIZE为4096(4kb)，为了提取低12位，即页内偏移量offset



将虚拟地址转换为虚拟页号
pub fn floor(&self) -> VirtPageNum {
    VirtPageNum(self.0 / PAGE_SIZE)
}



 Sv39 页表项（PTE）:
 53:10​ ​​物理页号
 9:0 十位标志位
 pub fn ppn(&self) -> PhysPageNum {
    (self.bits >> 10 & ((1usize << 44) - 1)).into()
}
取页表项去掉标志位得到ppn,又因为Sv39 模式下物理地址宽度为 44 位,所以&起来



物理页号转换为物理地址.into()
impl From<PhysPageNum> for PhysAddr {
    fn from(v: PhysPageNum) -> Self {
        Self(v.0 << 12)
    }
}

p.0 += offset;
-------------------------------------------------------------------------
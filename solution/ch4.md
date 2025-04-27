# 编程作业
-------------------------------------------------------------------------
## sys_get_time(1.0)
-------------------------------------------------------------------------
获得一个TimeVal：

--------------------------------------
1.取得当前硬件计时器值并转化为微秒数，然后拆分成TimeVal格式
```
let us = get_time_us();
let sec = us / 1_000_000;
let usec = us % 1_000_000;
```
--------------------------------------
查看 get_time_us 函数
```
#[allow(dead_code)]
pub fn get_time_us() -> usize {
    time::read() * MICRO_PER_SEC / CLOCK_FREQ
}
```
time::read()读取硬件计时器的​原始计数值，即时钟周期数
将​时钟周期数​​转换为​微秒数：
微秒数 = (时钟周期数 × 1_000_000) / 时钟频率

--------------------------------------
2.获取当前任务的用户态页表基地址
current_user_token()

--------------------------------------
```
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
```
--------------------------------------
3.现在有了具体的任务用户态基地址，需要把1的结果加到任务状态中,需要转化成内核可访问的地址
```
let ts = user_ptr_to_kernel_ref(current_user_token(), _ts);
```
-------------------------------------------------------------------------
在页表管理文件中编写新函数,使其实现3操作,对应两个元素，一个是usize，一个是T,返回T
完成用户态虚拟地址到内核态物理地址的安全翻译
是关键一步,手动实现翻译
// os/src/mm/page_table.rs
```
//虚拟地址->虚拟页号->物理页号->物理基地址->物理地址
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
```
-------------------------------------------------------------------------
对于 Sv39 虚拟内存模式，satp 的结构如下：
43:0 PPN 根页表的物理页号
62:44 ASID 地址空间标识符，用于区分不同的地址空间
63 	MODE 地址转换模式（Sv39 模式为 8）
因此satp左移44位-1是为了获得root_ppn
```
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
```


Sv39 是 RISC-V 的一种虚拟内存模式，支持 39 位虚拟地址空间
因此 (1 << VA_WIDTH_SV39) - 1 是取39位有效位
```
impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VA_WIDTH_SV39) - 1))
    }
}
```

```
let page_off = va.page_offset();
pub fn page_offset(&self) -> usize {
    self.0 & (PAGE_SIZE - 1)    // 0xfff
}
```
PAGE_SIZE为4096(4kb)，为了提取低12位，即页内偏移量offset



将虚拟地址转换为虚拟页号
```
pub fn floor(&self) -> VirtPageNum {
    VirtPageNum(self.0 / PAGE_SIZE)
}
```


 Sv39 页表项（PTE）:
 53:10​ ​​物理页号
 9:0 十位标志位
 ```
 pub fn ppn(&self) -> PhysPageNum {
    (self.bits >> 10 & ((1usize << 44) - 1)).into()
}
```
取页表项去掉标志位得到ppn,又因为Sv39 模式下物理地址宽度为 44 位,所以&起来


```
物理页号转换为物理地址.into()
impl From<PhysPageNum> for PhysAddr {
    fn from(v: PhysPageNum) -> Self {
        Self(v.0 << 12)
    }
}

p.0 += offset;
```
-------------------------------------------------------------------------
4.20
我发现translated_byte_buffer也可以实现用户空间虚拟地址转化为物理地址，使得地址合法内核访问
```
//虚拟地址->虚拟页号->物理页号->物理基地址->物理地址

/// Translate&Copy a ptr[u8] array with LENGTH len to a mutable u8 Vec through page table
// 将用户空间的虚拟地址缓冲区转换为内核可访问的物理内存切片集合
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    // 从token获取用户页表实例，用于地址转换
    let page_table = PageTable::from_token(token);
    
    // 计算用户空间缓冲区的起始和结束地址（虚拟地址）
    let mut start = ptr as usize;    // 转换为数值类型的起始地址
    let end = start + len;           // 结束地址 = 起始 + 长度
    let mut v = Vec::new();          // 存储最终生成的物理内存切片
    
    // 逐页处理用户缓冲区，直到覆盖全部长度
    while start < end {
        // 将当前起始地址转换为虚拟地址类型
        let start_va = VirtAddr::from(start);
        
        // 获取当前虚拟页号（VPN），例如0x1000对齐
        let mut vpn = start_va.floor();  // floor() 获取页起始地址对应的VPN
        
        // 将虚拟页号翻译为物理页号（PPN）
        let ppn = page_table.translate(vpn).unwrap().ppn();  // 若无法翻译会panic
        
        // 将VPN前进到下一页（例如0x1000 -> 0x2000）
        vpn.step();  // step() 方法将VPN增加一页大小
        
        // 计算当前页的结束地址：
        // 取下一页起始地址和用户缓冲区的结束地址中较小的值
        let mut end_va: VirtAddr = vpn.into();        // 转换为虚拟地址
        end_va = end_va.min(VirtAddr::from(end));     // 确保不超过总长度
        
        // 生成当前页的物理内存切片：
        if end_va.page_offset() == 0 {
            // 情况1：当前页正好结束在页边界
            // 切片从start_va的页内偏移到页末尾
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            // 情况2：当前页结束在中间位置
            // 切片从start_va的页内偏移到end_va的页内偏移
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        
        // 更新start为当前页的结束地址，处理下一页
        start = end_va.into();
    }
    
    v  // 返回所有物理页切片的集合
}
```
缓冲区是为了保证数据读入的合法不会越界和安全，做好了物理页切片的返回也就是实现了跨页处理
那么对应改写sys_get_time
```
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let token = current_user_token();
    let tv = TimeVal {
        sec : us / 1_000_000,
        usec : us % 1_000_000,
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &tv as *const _ as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };
    let mut bufs = translated_byte_buffer(token, _ts as *const u8, bytes.len());
    let mut copied = 0;
    for buf in &mut bufs {
        let len = buf.len().min(bytes.len() - copied);
        buf[..len].copy_from_slice(&bytes[copied..copied + len]);
        copied += len;
    }
    0
}
```
-------------------------------------------------------------------------
## sys_trace(1.0)
-------------------------------------------------------------------------
此外，由于本章我们有了地址空间作为隔离机制，所以 sys_trace 需要考虑一些额外的情况：
在读取（trace_request 为 0）时，如果对应地址用户不可见或不可读，则返回值应为 -1（isize 格式的 -1，而非 u8）。
在写入（trace_request 为 1）时，如果对应地址用户不可见或不可写，则返回值应为 -1（isize 格式的 -1，而非 u8）。

在ch3基础上修改

-------------------------------------------------------------------------
```
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token();
    match _trace_request {
        0 => {
            // id 应被视作 *const u8，读取地址处的值
            let buffers = translated_byte_buffer(token, _id as *const u8, 1);
            if let Some(buffer) = buffers.first() {
                buffer[0] as isize
            } else {
                -1 // 地址不可见或不可读
            }
        }
        1 => {
            // id 应被视作 *mut u8，写入 data 的最低字节
            let mut buffers = translated_byte_buffer(token, _id as *mut u8, 1);
            if let Some(buffer) = buffers.first_mut() {
                buffer[0] = (_data & 0xff) as u8;
                0 // 成功
            } else {
                -1 // 地址不可见或不可写
            }
        }
        2 => {
            // 查询当前任务调用编号为 id 的系统调用次数
            if _id >= MAX_SYSCALL_NUM {
                return -1;
            }
            let syscall_times = crate::task::get_sys_call_times();
            syscall_times[_id] as isize
        }
        _ => {
            -1
        }
    }
}
```
并进行相关get_sys_call_times系统调用配置

-------------------------------------------------------------------------
运行8/16
[kernel] Panicked at src/mm/heap_allocator.rs:12 Heap allocation error, layout = Layout { size: 16777216, align: 8 (1 << 3) }

出现堆分配错误:
在现有的代码里，把堆元数据搞坏（从而在下一次 alloc 的时候触发 “Heap allocation error”）的是你在内核里把用户给的指针当成一段连续的内存一次性拷贝，而这个拷贝又很有可能跨页（page）边界，结果写到了不该写的物理页上，把 buddy 分配器的元数据给砸烂了
最典型的例子就是 sys_get_time:
```
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let token = current_user_token();
    // 这个 user_ptr_to_kernel_ref 只处理了指针所在的那一页，
    // 然后你又一次性写入整个 TimeVal（16 byte），
    // 如果 _ts.page_offset() + 16 > 4096，就会跑到下一页去写！
    let ts = user_ptr_to_kernel_ref(token, _ts );
    *ts = TimeVal {
        sec : us / 1_000_000,
        usec : us % 1_000_000,
    };
    0    
}
```
当 TimeVal 恰好被分配在一个页的尾部，后半截会越界跑到下一页去写──那一页要么是别的映射，要么根本就没在你的页面表里，就直接把内核的物理内存给糟蹋了。被改坏的往往就是那块放 heap metadata 的区域，下一次 alloc 肯定会挂

正确的做法是：
1) 如果你要给用户空间写任意长度（不止一个机器字）的数据，不能用 user_ptr_to_kernel_ref 拿一个 &mut T 就直接赋。
2) 应该跟你做 sys_trace、sys_write／sys_read 那样，用 translated_byte_buffer(token, ptr, len) 拿到一系列分片（每个分片保证在同一个物理页里），然后逐字节或者逐切片 memcpy／copy。
举个 sketch：

```
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let tv = TimeVal { sec, usec };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &tv as *const _ as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };
    let mut bufs = translated_byte_buffer(token, _ts as *const u8, bytes.len());
    let mut copied = 0;
    for buf in &mut bufs {
        let len = buf.len().min(bytes.len() - copied);
        buf[..len].copy_from_slice(&bytes[copied..copied + len]);
        copied += len;
    }
    0
}
```

这样就绝不会越过页面边界去写，可以彻底消灭那种把 heap 元数据写炸的 bug，也就不会再蹦 “Heap allocation error” 了

-------------------------------------------------------------------------
改了还是不对,ch3的同步案例和ch4的trace01，unmap没过

-------------------------------------------------------------------------
受不了了，重写4/27

-------------------------------------------------------------------------
## sys_get_time
-------------------------------------------------------------------------
```
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let token = current_user_token();
    let ts = user_ptr_to_kernel_ref(token, _ts );
    *ts = TimeVal {
        sec : us / 1_000_000,
        usec : us % 1_000_000,
    };
    0    
}
```
```
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
```

1.0处有讲实现

-------------------------------------------------------------------------
## sys_trace
-------------------------------------------------------------------------
```
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token();
    match _trace_request {
        0 => {
            if _id > ((1 << 39) - 1) {
                return -1;
            }
            let pte = crate::task::find_pte_by_virtual_address(_id);
            if let Some(pte) = pte {
                if !pte.is_valid() || !pte.readable() {
                    return -1;
                }
            } else {
                return -1;
            }
            let buffers = translated_byte_buffer(token, _id as *const u8, 1);
            buffers[0][0] as isize 
        }
        1 => {
            if _id > ((1 << 39) - 1) {
                return -1;
            }
            let pte = crate::task::find_pte_by_virtual_address(_id);
            if let Some(pte) = pte {
                if !pte.is_valid() || !pte.writable() {
                    return -1;
                }
            } else {
                return -1;
            }
            let mut buffers = translated_byte_buffer(token, _id as *mut u8, 1);
            buffers[0][0] = (_data & 0xff) as u8;
            0
        }
        2 => {
            if _id >= MAX_SYSCALL_NUM {
                return -1;  
            }
            crate::task::get_syscall_count(_id) as isize
        }
        _ => {
            -1
        }
    }
}
```

0：读取用户内存

检查目标地址 _id 是否超出 ​​sv39 虚拟地址范围​​(<=2^39-1)
```
 if _id > ((1 << 39) - 1) {
                return -1;
            }
```

通过页表项（PTE）检查地址是否已映射且​​可读​​，若无效则返回错误
```
let pte = crate::task::find_pte_by_virtual_address(_id);
if let Some(pte) = pte {
    if !pte.is_valid() || !pte.writable() {
        return -1;
    }
}  
else {
    return -1;
 }           
```
目的是检查用户空间某个地址的可读性并返回其值，而非读取连续内存

因此使用 translated_byte_buffer 将用户地址转换为内核安全访问的缓冲区，并返回首个字节的值,是为了安全性

安全性​​：单字节操作能最小化内核与用户空间的数据交互，降低越界访问风险（例如避免意外读取相邻敏感数据）

仅需验证和转换目标地址的一字节，减少页表查询和内存拷贝开销

translated_byte_buffer 的结构​​：
该函数（假设为 rCore 的实现）会将用户空间的 ​​不连续内存区间​​ 转换为内核可安全访问的 ​​缓冲区数组​​，其返回值通常为 Vec<&mut [u8]> 或类似结构。

​​第一维 [0]​​：表示用户地址可能跨多个物理页，这里取第一个物理页的缓冲区。

​​第二维 [0]​​：表示目标地址在该物理页缓冲区中的偏移量。


```
 let buffers = translated_byte_buffer(token, _id as *const u8, 1);
            buffers[0][0] as isize     
```
1：写入用户内存

同理，写入只有这个区别
```
 buffers[0][0] = (_data & 0xff) as u8
```

2：​​查询系统调用次数​​
```
crate::task::get_syscall_count(_id) as isize
```

***有个关键点是***:
计数时，不同的syscall都会increase，那么需要在syscall类里加入这个函数
//syscall/mod.rs
```
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    crate::task::increase_sys_call(syscall_id);
    match syscall_id {
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(args[0] as *mut TimeVal, args[1]),
        SYSCALL_TRACE => sys_trace(args[0], args[1], args[2]),
        SYSCALL_MMAP => sys_mmap(args[0], args[1], args[2]),
        SYSCALL_MUNMAP => sys_munmap(args[0], args[1]),
        SYSCALL_SBRK => sys_sbrk(args[0] as i32),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
```

-------------------------------------------------------------------------
## find_pte_by_virtual_address
-------------------------------------------------------------------------
//task/mod.rs
```
/// Find PageTableEntry by VirtPageNum, create a frame for a 4KB page table if not exist
pub fn find_pte_by_virtual_address(virtual_address: usize) ->  Option<PageTableEntry> {
    TASK_MANAGER.find_pte_by_virtual_address(virtual_address)
}
```
在impl TaskManager方法中给出vaddr->pte的映射函数
```
/// Virtualaddr->pte
fn find_pte_by_virtual_address(&self, virtual_address: usize) -> Option<PageTableEntry> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let va = VirtAddr::from(virtual_address);
        let vpn = va.floor();
        let pte = inner.tasks[current].memory_set.find_pte(vpn);
        if let Some(x) = pte {
            return Some(x.clone());
        }
        else {
            return None;
        }
}

/// VirtPageNum->pte
fn find_pte(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
    let inner = self.inner.exclusive_access();
    let current = inner.current_task;
    let pte = inner.tasks[current].memory_set.find_pte(vpn);
    if let Some(x) = pte {
        return Some(x.clone());
    }
    else {
        return None;
    }
}    
```


-------------------------------------------------------------------------
详细介绍函数：
```
let inner = self.inner.exclusive_access();// 类似于锁，独占访问权防止并发修改
let current = inner.current_task;// 获取当前正在运行的进程/任务的 ID
```
将原始地址 usize 转换为类型化的虚拟地址 VirtAddr

计算虚拟地址对应的​虚拟页号​​，即去掉页内偏移（VPN = address / PAGE_SIZE）
```
let va = VirtAddr::from(virtual_address);
let vpn = va.floor();
```
由于

exclusive_access() 函数通常用于多线程环境（如 UPSafeCell），返回的 inner 是一个 ​​受保护的临时可变访问​​
若返回内部数据的引用，会破坏 Rust 的借用规则：​​多个线程可能同时持有 PTE 引用，而 inner 的锁已被释放​​，导致数据竞争

因此

查找页表项find_pte,最后返回的是克隆的pte ; 代码在保证安全性的同时，避免了复杂的生命周期管理和锁竞争问题

-------------------------------------------------------------------------
## find_pte
-------------------------------------------------------------------------
在内存管理中被包装

//mm/memory_set.rs

impl MemorySet
```
/// Finds the page table entry corresponding to the given virtual page number.
pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
    self.page_table.find_pte(vpn)    
}
```
页表page_table.rs中有他的实现:(详细可以深入理解)
```
  /// Find PageTableEntry by VirtPageNum
    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }  
```

-------------------------------------------------------------------------
## mmap
-------------------------------------------------------------------------
先验证port：只能地三位且有值
```
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    if _len == 0 {
        return 0;
    }
    //port只能是0x1,0x3,0x5,0x7
    // 0x1: read
    // 0x3: read and write
    // 0x5: read and execute
    // 0x7: read, write and execute
    if _port & !0x7 != 0 || _port & 0x7 == 0 {
        return -1;
    }
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    to_mmap(_start, _len, _port)
}
```
//task/mod.rs
```
/// select_cur_task_to_mmap
pub fn to_mmap(start: usize, len: usize, port: usize) -> isize {
    TASK_MANAGER.mmap(start, len, port)
}
```
在任务管理方法中：
```
fn mmap(&self, start: usize, len: usize, port: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current_task = inner.current_task;
        inner.tasks[current_task].memory_set.mmap(start, len, port)
    }
```
在内存管理中：
```
/// mmap
    pub fn mmap(&mut self, start: usize, len: usize, port: usize) -> isize {
        let va_start: VirtAddr = start.into(); // 接收start虚拟地址
        if !va_start.aligned() {
            debug!("unmap fail don't aligned");
            return -1;
        }// start虚拟地址必须是4k对齐，检验4k对齐
        if start + len > MEMORY_END {
            debug!("unmap fail out of memory");
            return -1;
        }// 检验是否越界
        let mut va_start: VirtPageNum = va_start.into();// 将虚拟地址转成虚拟页号
        let mut flags = PTEFlags::empty();
        if port & 0b0000_0001 != 0 {
            flags |= PTEFlags::R;
        }

        if port & 0b0000_0010 != 0 {
            flags |= PTEFlags::W;
        }

        if port & 0b0000_0100 != 0 {
            flags |= PTEFlags::X;
        }
        flags |= PTEFlags::U;
        flags |= PTEFlags::V;
        // 取标志位
        if flags.is_empty() {
            debug!("unmap fail no permission");
            return -1;
        }

        let va_end: VirtAddr = (start + len).into();// 找到结束虚拟地址
        let va_end: VirtPageNum = va_end.ceil();// 向上取整，找到结束虚拟页号
        if va_start >= va_end {
            debug!("unmap fail start >= end");
            return -1;
        }

        while va_start != va_end {
            if let Some(pte) = self.page_table.translate(va_start) {// 生成页表项
                if pte.is_valid() {
                    return -1;// 如果已经映射了，返回-1
                }
            }
            if let Some(ppn) = frame_alloc() {
                self.page_table.map(va_start, ppn.ppn, flags);
                self.map_tree.insert(va_start, ppn);
            } else {
                return -1;
            }
            va_start.step();
        }
        0
    }

```
-------------------------------------------------------------------------
## unmmap
-------------------------------------------------------------------------
```
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    to_munmap(_start, _len)
}
```
```
/// select_cur_task_to_mmap
pub fn to_munmap(start: usize, len: usize) -> isize {
    TASK_MANAGER.munmap(start, len)
}
```
```
fn munmap(&self, start: usize, len: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current_task = inner.current_task;
        inner.tasks[current_task].memory_set.unmmap(start, len)
    }
```
```
/// munmap
    pub fn unmmap(&mut self, start: usize, len: usize) -> isize {
        let va_start: VirtAddr = start.into();
        if !va_start.aligned() {
            debug!("unmap fail don't aligned");
            return -1;
        }// 检查4k对齐
        let mut va_start: VirtPageNum = va_start.into();// 虚拟地址转成虚拟页号

        let va_end: VirtAddr = (start + len).into();// 找到结束虚拟地址
        let va_end: VirtPageNum = va_end.ceil();// 向上取整，找到结束虚拟页号

        while va_start != va_end {
            if let Some(unpte) = self.page_table.translate(va_start) {
                if !unpte.is_valid() {
                    debug!("unmap on no map vpn");
                    return -1;
                }// 没有映射，则不需要unmap
            } else {
                return -1;
            }
            self.page_table.unmap(va_start);
            self.map_tree.remove(&va_start);
            va_start.step();
        }
        0
    }
```
-------------------------------------------------------------------------
cd ci-user && make test CHAPTER=4

16/16
![alt text](image-7.png)
-------------------------------------------------------------------------
# 问答作业
-------------------------------------------------------------------------
## 请列举 SV39 页表页表项的组成，描述其中的标志位有何作用？
-------------------------------------------------------------------------
SV39 页表项组成及标志位作用

SV39 页表项（PTE）包含以下字段：
```
​​物理页号（PPN）​​：44 位，表示物理页的基址。
​​标志位​​：共 10 位，包括：
​​V（有效位）​​：表示该 PTE 是否有效。
​​R（可读）​​、​​W（可写）​​、​​X（可执行）​​：权限控制。
​​U（用户位）​​：用户态是否可访问。
​​A（访问位）​​：记录该页是否被访问过。
​​D（修改位）​​：记录该页是否被修改过。
​​RSW（保留位）​​：供操作系统自定义使用。
```
-------------------------------------------------------------------------
## 请问哪些异常可能是缺页导致的？发生缺页时，描述相关重要寄存器的值，上次实验描述过的可以简略

缺页指的是进程访问页面时页面不在页表中或在页表中无效的现象，此时 MMU 将会返回一个中断， 告知 os 进程内存访问出了问题。os 选择填补页表并重新执行异常指令或者杀死进程。

-------------------------------------------------------------------------
```
缺页导致的异常
可能触发缺页的异常包括：
​​指令缺页​​（异常码 12）：取指令时目标页无效。
​​加载缺页​​（异常码 13）：读数据时目标页无效。
​​存储缺页​​（异常码 15）：写数据时目标页无效。
```
```
缺页时的寄存器状态
​​stval​​：保存触发缺页的虚拟地址。
​​scause​​：记录异常原因（如 12、13、15）。
```
-------------------------------------------------------------------------
## 以下做法这样做有哪些好处(Lazy策略)

缺页有两个常见的原因，其一是 Lazy 策略，也就是直到内存页面被访问才实际进行页表操作。 比如，一个程序被执行时，进程的代码段理论上需要从磁盘加载到内存。但是 os 并不会马上这样做， 而是会保存 .text 段在磁盘的位置信息，在这些代码第一次被执行时才完成从磁盘的加载操作

-------------------------------------------------------------------------
```
Lazy 策略的优势
​​减少初始开销​​：避免立即分配未使用的内存。
​​节省资源​​：仅加载实际访问的页面，减少磁盘和内存操作。
```
其实，我们的 mmap 也可以采取 Lazy 策略，比如：一个用户进程先后申请了 10G 的内存空间， 然后用了其中 1M 就直接退出了。按照现在的做法，我们显然亏大了，进行了很多没有意义的页表操作

-------------------------------------------------------------------------
## 处理 10G 连续的内存页面，对应的 SV39 页表大致占用多少内存 (估算数量级即可)？
-------------------------------------------------------------------------
SV39 页表占用估算
10G 内存需映射 10 * 2^30 / 4K = 2.5M 页，每页对应 8 字节 PTE，总大小约 2.5M * 8B = 20MB。三级页表额外开销约 8KB，总量级为 ​​几十 MB​​。

-------------------------------------------------------------------------
## 请简单思考如何才能实现 Lazy 策略，缺页时又如何处理？描述合理即可，不需要考虑实现。
-------------------------------------------------------------------------
```
Lazy 策略实现
​​延迟分配​​：初始标记 PTE 为无效，记录需求信息（如大小）。
​​缺页处理​​：分配物理页，更新 PTE 为有效并设置权限，重新执行指令。
```
缺页的另一个常见原因是 swap 策略，也就是内存页面可能被换到磁盘上了，导致对应页面失效

-------------------------------------------------------------------------
## 此时页面失效如何表现在页表项(PTE)上?
-------------------------------------------------------------------------
```
Swap 策略的 PTE 表现
PTE 的 ​​V 位清零​​，部分字段（如物理页号）可能记录磁盘位置（如交换区偏移）。
```
-------------------------------------------------------------------------
## 双页表与单页表
为了防范侧信道攻击，我们的 os 使用了双页表。但是传统的设计一直是单页表的，也就是说， 用户线程和对应的内核线程共用同一张页表，只不过内核对应的地址只允许在内核态访问。 (备注：这里的单/双的说法仅为自创的通俗说法，并无这个名词概念，详情见 KPTI )

-------------------------------------------------------------------------
```
单页表与双页表
​​单页表更换方式​​：通过修改 satp 寄存器切换进程页表。
​​用户态访问控制​​：内核页面的 PTE 中 ​​U 位为 0​​，用户态访问触发异常。
​​单页表优势​​：
​​上下文切换开销低​​：无需频繁切换页表。
​​TLB 效率高​​：内核与用户共享页表，减少刷新。
​​双页表切换时机​​：用户态/内核态切换时更换页表；单页表仅在进程切换时更换。
```
-------------------------------------------------------------------------
# 荣誉准则
-------------------------------------------------------------------------
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

《你交流的对象说明》

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

《你参考的资料说明》

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。


-------------------------------------------------------------------------
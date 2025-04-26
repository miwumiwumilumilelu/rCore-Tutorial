# 编程作业
-------------------------------------------------------------------------
## sys_get_time
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
## sys_trace
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
## ch3的同步案例和ch4的trace01，unmap没过

-------------------------------------------------------------------------


-------------------------------------------------------------------------

-------------------------------------------------------------------------
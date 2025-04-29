# 编程作业
-------------------------------------------------------------------------
## sys_get_time迁移
-------------------------------------------------------------------------
```
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
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
/// Translate a ptr[u8] array through page table and return a mutable reference of T
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
## sys_mmap sys_munmap迁移
-------------------------------------------------------------------------
```
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
```
为了适应ch5框架，安全地获取并修改当前任务（如线程或异步任务）的内部状态

current_task() 函数可能返回 Option<Task> 或 Result<Task>，表示当前正在执行的任务。unwrap() 用于提取 Task 实例，假设此时必有任务存在，否则会触发 panic（需确保在安全上下文中使用）。

inner_exclusive_access() 是任务结构体提供的方法，用于获取其内部数据的​​独占访问权​​，防止数据竞争
```
/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_mmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
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
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    // 这里的inner是一个MutexGuard，表示对当前任务的独占访问
    // 通过inner获取当前任务的内存集
    inner.memory_set.mmap(_start, _len, _port);
    0
}


/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_munmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    // 这里的inner是一个MutexGuard，表示对当前任务的独占访问
    // 通过inner获取当前任务的内存集
    inner.memory_set.unmmap(_start, _len);
    0
}

```
/mm/memory_set.rs

MemorySet方法中添加:
```
    /// map a range of virtual memory.
    pub fn mmap(&mut self, start: usize, len: usize, port: usize) -> isize{
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

    /// Unmap a range of virtual memory.
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
## sys_spawn
-------------------------------------------------------------------------

-------------------------------------------------------------------------
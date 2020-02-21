// Copyright (c) 2020 Alex Chi
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use super::{TrapFrame, Context, Register, ContextRegisters};
use crate::{page, panic, info, warn};
use crate::symbols::*;
use crate::mem;
use crate::arch;
use crate::println;
use crate::trap::usertrapret;
use alloc::boxed::Box;
use crate::process::{put_back_proc, my_proc, PROCS_POOL, PROCS_POOL_SLEEP, my_cpu, sched, ProcInPool};
use crate::page::{Page, Table, EntryAttributes};
use crate::process::Register::a0;
use crate::fs;
use crate::jump::*;
use crate::spinlock::{Mutex, MutexGuard};

#[derive(PartialEq)]
#[derive(Debug)]
pub enum ProcessState {
    UNUSED,
    SLEEPING,
    RUNNABLE,
    RUNNING,
    ZOMBIE,
}

#[repr(C)]
#[repr(align(4096))]
pub struct Process {
    pub pgtable: Box<page::Table>,
    pub trapframe: Box<TrapFrame>,
    pub context: Box<Context>,
    pub state: ProcessState,
    pub kstack: usize,
    pub kstack_sp: usize,
    pub pid: i32,
    pub channel: usize,
    pub drop_on_put_back: Option<MutexGuard<'static, ()>>,
}

impl Process {
    pub fn new(pid: i32) -> Self {
        Self::from_exist(pid, box page::Table::new(), box TrapFrame::zero())
    }

    pub fn from_exist(pid: i32, pgtable: Box<Table>, trapframe: Box<TrapFrame>) -> Self {
        if pid < 0 {
            panic!("invalid pid");
        }

        let kstack = mem::ALLOC().lock().allocate(PAGE_SIZE * 1024) as usize;

        let mut p = Self {
            trapframe,
            pgtable,
            context: box Context::zero(),
            state: ProcessState::UNUSED,
            kstack: kstack,
            kstack_sp: kstack + PAGE_SIZE * 1024,
            pid,
            channel: 0,
            drop_on_put_back: None,
        };

        // map trampoline
        p.pgtable.kernel_map(
            TRAMPOLINE_START,
            TRAMPOLINE_TEXT_START(),
            page::EntryAttributes::RX as usize,
        );

        let trapframe = &*p.trapframe as *const _ as usize;
        // map trapframe
        p.pgtable.kernel_map(
            TRAPFRAME_START,
            trapframe,
            page::EntryAttributes::RW as usize,
        );
        p.context.regs[ContextRegisters::ra as usize] = forkret as usize;
        p.context.regs[ContextRegisters::sp as usize] = p.kstack + PAGE_SIZE;

        p
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _kstack = unsafe { Box::from_raw(self.kstack as *mut Page) };
    }
}

#[no_mangle]
pub extern "C" fn forkret() -> ! {
    usertrapret()
}

/// binary code of user/src/initcode.S
/// This file will be compiled to elf, and then
/// be stripped with objdump, as specified in Makefile.
fn init_code() -> &'static [u8] {
    #[cfg(debug_assertions)]
        let x = include_bytes!("../../../target/riscv64gc-unknown-none-elf/debug/initcode");
    #[cfg(not(debug_assertions))]
        let x = include_bytes!("../../../target/riscv64gc-unknown-none-elf/release/initcode");
    x
}

/// Put init process into `PROCS_POOL`
pub fn init_proc() {
    let mut p = Process::new(0);
    // map init code
    let content = init_code();
    let mut page = Page::new();
    page.data[0..content.len()].copy_from_slice(content);
    p.pgtable.map(0, page, EntryAttributes::URX as usize);
    // map user stack
    let sp = map_stack(&mut p.pgtable, 0x80001000);
    p.trapframe.epc = 0;
    p.trapframe.regs[Register::sp as usize] = sp;
    p.state = ProcessState::RUNNABLE;
    put_back_proc(box p);
}

pub fn find_available_pid() -> Option<i32> {
    let pool = PROCS_POOL.lock();
    for i in 0..NMAXPROCS {
        match &pool[i] {
            ProcInPool::NoProc => return Some(i as i32),
            _ => {}
        }
    }
    None
}

pub fn fork() -> i32 {
    let p = my_proc();
    let f_pid = find_available_pid();
    if f_pid.is_none() {
        panic!("pid unavailable");
    }
    let f_pid = f_pid.unwrap();
    let pgtable = p.pgtable.clone();
    let trapframe = box *p.trapframe.clone();
    let mut fork_p = Process::from_exist(f_pid, pgtable, trapframe);
    fork_p.trapframe.regs[a0 as usize] = 0;
    fork_p.state = ProcessState::RUNNABLE;
    put_back_proc(box fork_p);
    f_pid
}

pub const USER_STACK_PAGE: usize = 4;

/// map user stack in `pgtable` at `stack_begin` and returns `sp`
pub fn map_stack(pgtable: &mut Table, stack_begin: usize) -> usize {
    for i in 0..USER_STACK_PAGE {
        let stack = page::Page::new();
        pgtable.map(
            stack_begin + i * PAGE_SIZE,
            stack,
            page::EntryAttributes::URW as usize,
        );
    }

    stack_begin + PAGE_SIZE * USER_STACK_PAGE
}

/// exec syscall
pub fn exec(path: &str) {
    let p = my_proc();
    let content = fs::get_file(path);
    p.pgtable.unmap_user();
    let entry = crate::elf::parse_elf(
        content,
        &mut p.pgtable,
    );
    // map user stack
    let sp = map_stack(&mut p.pgtable, 0x80001000);
    p.trapframe.epc = entry as usize;
    p.trapframe.regs[Register::sp as usize] = sp;
}

/// exit syscall
pub fn exit(status: i32) -> ! {
    {
        let p = my_proc();
        if p.pid == 0 {
            panic!("init exiting");
        }
        p.state = ProcessState::ZOMBIE;
    }
    arch::intr_off();
    sched();
    unreachable!();
}


/// put this process into sleep state
pub fn sleep<'a, T, U>(channel: *const T, lck: MutexGuard<'a, U>) -> MutexGuard<'a, U> {
    let p = my_proc();
    p.channel = channel as *const _ as usize;
    p.state = ProcessState::SLEEPING;

    // set proc in proc pool as being slept, avoiding lost-wakeup issue
    {
        let mut pool = PROCS_POOL.lock();
        let p_in_pool = &mut pool[p.pid as usize];
        match p_in_pool {
            ProcInPool::Scheduled => {}
            _ => panic!("invalid proc pool state")
        }
        *p_in_pool = ProcInPool::BeingSlept;
    }
    p.drop_on_put_back = Some(PROCS_POOL_SLEEP.lock());

    // temporarily unlock spinlock
    let weak_lock = lck.into_weak();

    sched();

    p.channel = 0;

    return weak_lock.into_guard();
}

/// wakeup process on channel
pub fn wakeup<T>(channel: *const T) {
    // info!("wakeup {:x}", channel as usize);
    let channel = channel as *const _ as usize;
    let mut pool = PROCS_POOL.lock();
    let mut i = 0;
    while i < NMAXPROCS {
        match &mut pool[i] {
            ProcInPool::Pooling(p) => {
                // info!("channel of {} = {:x}", p.pid, p.channel);
                if p.state == ProcessState::SLEEPING && p.channel == channel {
                    p.state = ProcessState::RUNNABLE;
                }
                i += 1;
            }
            ProcInPool::BeingSlept => {
                let weak_lock = pool.into_weak();
                PROCS_POOL_SLEEP.lock();
                pool = weak_lock.into_guard();
            }
            _ => { i += 1; }
        }
    }
}

use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

use embassy_executor::{Executor, InterruptExecutor, SendSpawner, Spawner};

use crate::{
    interrupt::{Priority, set_priority},
    pac::{self, interrupt},
    system,
};

const KEY_INTERRUPT: pac::Interrupt = pac::Interrupt::UART4;
const DEVICE_INTERRUPT_WORDS: usize = 2;
const SYSTICK_OFF: u32 = 0;
const ICSR_PENDSTCLR: u32 = 1 << 25;
const ICSR_PENDSVCLR: u32 = 1 << 27;
const AIRCR_VECTKEY_NO_SUBPRIORITY: u32 = 0x05FA << 16;

static STARTED: AtomicBool = AtomicBool::new(false);
static KEYS: InterruptExecutor = InterruptExecutor::new();
static mut THREAD: MaybeUninit<Executor> = MaybeUninit::uninit();

pub fn run(init: impl FnOnce(Spawner, SendSpawner)) -> ! {
    if STARTED.swap(true, Ordering::AcqRel) {
        system::reset();
    }
    settle_core();
    set_priority(KEY_INTERRUPT, Priority::KEYS);
    let keys = KEYS.start(KEY_INTERRUPT);
    let thread = &raw mut THREAD;
    // SAFETY: `STARTED` lets exactly one call reach this point, so this is the only reference to
    // `THREAD` that ever exists.
    let thread = unsafe { (*thread).write(Executor::new()) };
    thread.run(|spawner| init(spawner, keys))
}

fn settle_core() {
    // SAFETY: `STARTED` lets this run once, in thread mode, before this firmware has enabled an
    // interrupt or entered a critical section. The writes stop the system tick, mask and unpend
    // every device interrupt, select the priority grouping without sub-priorities that
    // `Priority::to_nvic` and the BASEPRI critical section assume, and unmask interrupts, so that
    // nothing the bootloader left enabled or pending reaches a vector this firmware does not own.
    unsafe {
        let cpu = cortex_m::Peripherals::steal();
        cpu.SYST.csr.write(SYSTICK_OFF);
        for (enable, pending) in cpu
            .NVIC
            .icer
            .iter()
            .zip(cpu.NVIC.icpr.iter())
            .take(DEVICE_INTERRUPT_WORDS)
        {
            enable.write(u32::MAX);
            pending.write(u32::MAX);
        }
        cpu.SCB.icsr.write(ICSR_PENDSTCLR | ICSR_PENDSVCLR);
        cpu.SCB.aircr.write(AIRCR_VECTKEY_NO_SUBPRIORITY);
        cortex_m::register::basepri::write(0);
        cortex_m::interrupt::enable();
    }
}

#[interrupt]
fn UART4() {
    // SAFETY: `run` unmasks this interrupt only through `KEYS.start`, which initialises the
    // executor first; nothing else uses the vector or the UART4 peripheral.
    unsafe { KEYS.on_interrupt() };
}

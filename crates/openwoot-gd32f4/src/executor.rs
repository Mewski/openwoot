use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

use cortex_m::peripheral::SCB;
use embassy_executor::{Executor, InterruptExecutor, SendSpawner, Spawner};

use crate::{
    interrupt::{Priority, set_priority},
    pac::{self, interrupt},
    system,
};

const KEY_INTERRUPT: pac::Interrupt = pac::Interrupt::UART4;
const AIRCR_VECTKEY_NO_SUBPRIORITY: u32 = 0x05FA << 16;

static STARTED: AtomicBool = AtomicBool::new(false);
static KEYS: InterruptExecutor = InterruptExecutor::new();
static mut THREAD: MaybeUninit<Executor> = MaybeUninit::uninit();

pub fn run(init: impl FnOnce(Spawner, SendSpawner)) -> ! {
    if STARTED.swap(true, Ordering::AcqRel) {
        system::reset();
    }
    use_every_priority_bit_for_preemption();
    set_priority(KEY_INTERRUPT, Priority::KEYS);
    let keys = KEYS.start(KEY_INTERRUPT);
    let thread = &raw mut THREAD;
    // SAFETY: `STARTED` lets exactly one call reach this point, so this is the only reference to
    // `THREAD` that ever exists.
    let thread = unsafe { (*thread).write(Executor::new()) };
    thread.run(|spawner| init(spawner, keys))
}

fn use_every_priority_bit_for_preemption() {
    // SAFETY: `STARTED` lets this run once, in thread mode, before this firmware has enabled an
    // interrupt. The write carries the register key and selects priority group 0, so all four
    // implemented priority bits pre-empt, as `Priority::to_nvic` and the BASEPRI critical section
    // assume; it requests no reset and clears no active state.
    unsafe { (*SCB::PTR).aircr.write(AIRCR_VECTKEY_NO_SUBPRIORITY) };
}

#[interrupt]
fn UART4() {
    // SAFETY: `run` unmasks this interrupt only through `KEYS.start`, which initialises the
    // executor first; nothing else uses the vector or the UART4 peripheral.
    unsafe { KEYS.on_interrupt() };
}

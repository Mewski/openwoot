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

static STARTED: AtomicBool = AtomicBool::new(false);
static KEYS: InterruptExecutor = InterruptExecutor::new();
static mut THREAD: MaybeUninit<Executor> = MaybeUninit::uninit();

pub fn run(init: impl FnOnce(Spawner, SendSpawner)) -> ! {
    if STARTED.swap(true, Ordering::AcqRel) {
        system::reset();
    }
    set_priority(KEY_INTERRUPT, Priority::KEYS);
    let keys = KEYS.start(KEY_INTERRUPT);
    let thread = &raw mut THREAD;
    // SAFETY: `STARTED` lets exactly one call reach this point, so this is the only reference to
    // `THREAD` that ever exists.
    let thread = unsafe { (*thread).write(Executor::new()) };
    thread.run(|spawner| init(spawner, keys))
}

#[interrupt]
fn UART4() {
    // SAFETY: `run` unmasks this interrupt only through `KEYS.start`, which initialises the
    // executor first; nothing else uses the vector or the UART4 peripheral.
    unsafe { KEYS.on_interrupt() };
}

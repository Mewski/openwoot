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
const DEVICE_INTERRUPT: pac::Interrupt = pac::Interrupt::UART3;
const LIGHTING_INTERRUPT: pac::Interrupt = pac::Interrupt::USART2;
const AIRCR_VECTKEY_NO_SUBPRIORITY: u32 = 0x05FA << 16;

static STARTED: AtomicBool = AtomicBool::new(false);
static KEYS: InterruptExecutor = InterruptExecutor::new();
static DEVICE: InterruptExecutor = InterruptExecutor::new();
static LIGHTING: InterruptExecutor = InterruptExecutor::new();
static mut THREAD: MaybeUninit<Executor> = MaybeUninit::uninit();

#[derive(Clone, Copy)]
pub struct Spawners {
    pub thread: Spawner,
    pub keys: SendSpawner,
    pub device: SendSpawner,
    pub lighting: SendSpawner,
}

pub fn run(init: impl FnOnce(Spawners)) -> ! {
    if STARTED.swap(true, Ordering::AcqRel) {
        system::reset();
    }
    use_every_priority_bit_for_preemption();
    set_priority(KEY_INTERRUPT, Priority::KEYS);
    set_priority(DEVICE_INTERRUPT, Priority::DEVICE);
    set_priority(LIGHTING_INTERRUPT, Priority::LIGHTING);
    let keys = KEYS.start(KEY_INTERRUPT);
    let device = DEVICE.start(DEVICE_INTERRUPT);
    let lighting = LIGHTING.start(LIGHTING_INTERRUPT);
    let thread = &raw mut THREAD;
    // SAFETY: `STARTED` lets exactly one call reach this point, so this is the only reference to
    // `THREAD` that ever exists.
    let thread = unsafe { (*thread).write(Executor::new()) };
    thread.run(|thread| {
        init(Spawners {
            thread,
            keys,
            device,
            lighting,
        });
    })
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

#[interrupt]
fn UART3() {
    // SAFETY: `run` unmasks this interrupt only through `DEVICE.start`, which initialises the
    // executor first; nothing else uses the vector or the UART3 peripheral.
    unsafe { DEVICE.on_interrupt() };
}

#[interrupt]
fn USART2() {
    // SAFETY: `run` unmasks this interrupt only through `LIGHTING.start`, which initialises the
    // executor first; nothing else uses the vector or the USART2 peripheral.
    unsafe { LIGHTING.on_interrupt() };
}

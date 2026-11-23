use embassy_futures::select::select4;
use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};

use super::KeyNotices;
use crate::{
    diagnostics::Diagnostics,
    keys::Key,
    lighting::{
        self, HostControl, KeyEvent, KeyEvents, KeyTravel, LedMatrix, Lightbar, OutputChange,
        Signals,
    },
    profile,
    usb::HostState,
};

pub struct Lighting<
    'd,
    M: RawMutex,
    H,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> {
    pub lighting: &'d Mutex<M, lighting::Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>>,
    pub events: &'d KeyEvents,
    pub notices: &'d KeyNotices,
    pub host: &'d HostState,
    pub host_control: &'d HostControl,
    pub diagnostics: &'d Diagnostics,
    pub profile: &'d profile::Shared<M>,
}

impl<M, H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Lighting<'_, M, H, ROWS, COLUMNS, PAGES, LENGTH>
where
    M: RawMutex,
    H: Lightbar + Signals,
{
    const FAILED_STEPS_BEFORE_INITIALIZE: u8 = 3;
    const POLL_INTERVAL: Duration = Duration::from_millis(4);

    pub async fn run(&self, matrix: &mut impl LedMatrix, input: &impl KeyTravel) -> ! {
        let mut initialized = false;
        let mut failed_steps = 0_u8;
        loop {
            if !initialized {
                initialized = self.initialize(matrix).await;
            }
            let delivered = self
                .step(
                    matrix,
                    input,
                    now_ms(),
                    initialized && self.host.is_active(),
                )
                .await;
            failed_steps = if delivered {
                0
            } else {
                failed_steps.saturating_add(1)
            };
            if failed_steps >= Self::FAILED_STEPS_BEFORE_INITIALIZE {
                initialized = false;
                failed_steps = 0;
            }
            select4(
                self.events.ready_to_receive(),
                self.notices.ready(),
                self.host_control.ready(),
                Timer::after(Self::POLL_INTERVAL),
            )
            .await;
        }
    }

    pub async fn initialize(&self, matrix: &mut impl LedMatrix) -> bool {
        let initialized = matrix.initialize().await.is_ok();
        if initialized {
            self.lighting.lock().await.set_output(false);
        }
        self.diagnostics.set_lighting_initialized(initialized);
        initialized
    }

    pub async fn step(
        &self,
        matrix: &mut impl LedMatrix,
        input: &impl KeyTravel,
        now_ms: u32,
        usb_active: bool,
    ) -> bool {
        let mut lighting = self.lighting.lock().await;
        if self.host_control.take_session_end() {
            self.profile.host_signals().end_session();
        }
        self.profile.apply(&mut lighting, now_ms);
        while let Ok(activity) = self.events.try_receive() {
            lighting.key_event(activity.key, activity.event, activity.at_ms);
            lighting.hooks_mut().key_event(activity.key, activity.event);
            if activity.event == KeyEvent::Pressed {
                lighting.reset_idle_timer(now_ms);
            }
        }
        self.notices.apply(&mut lighting, now_ms);
        self.host_control.apply(&mut lighting, now_ms);
        let deepest = deepest_travel::<ROWS, COLUMNS>(input);
        lighting.hold_idle_timer(deepest, now_ms);
        let hooks = lighting.hooks_mut();
        hooks.travel_changed(deepest);
        hooks.tick(now_ms);
        if let Some(level) = lighting.take_brightness_save(now_ms) {
            self.profile.save_brightness(level);
        }
        let switched = match lighting.set_output(usb_active) {
            Some(OutputChange::Wake) => {
                let running = matrix.set_running(true).await.is_ok();
                if running {
                    lighting.wake(now_ms);
                    lighting.invalidate();
                } else {
                    lighting.set_output(false);
                }
                running
            }
            Some(OutputChange::ShutDown) => {
                let stopped = matrix.set_running(false).await.is_ok();
                if !stopped {
                    lighting.set_output(true);
                }
                stopped
            }
            None => true,
        };
        self.diagnostics
            .set_lighting_active(lighting.output_enabled());
        if !usb_active || !lighting.output_enabled() {
            return switched;
        }
        lighting.update(now_ms, input);
        self.diagnostics.frame_rendered();
        while let Some(write) = lighting.next_write() {
            let written = matrix
                .write(write.bank, write.first_register, write.values())
                .await;
            self.diagnostics.page_written(&written);
            if written.is_err() {
                lighting.invalidate();
                return false;
            }
        }
        true
    }
}

fn now_ms() -> u32 {
    u32::try_from(Instant::now().as_millis() & u64::from(u32::MAX)).unwrap_or_default()
}

fn deepest_travel<const ROWS: usize, const COLUMNS: usize>(input: &impl KeyTravel) -> u16 {
    (0..ROWS)
        .flat_map(|row| (0..COLUMNS).map(move |column| (row, column)))
        .filter_map(|(row, column)| {
            Some(Key::new(
                u8::try_from(row).ok()?,
                u8::try_from(column).ok()?,
            ))
        })
        .map(|key| input.travel(key))
        .max()
        .unwrap_or(0)
}

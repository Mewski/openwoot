use embassy_futures::select::{Either, select, select4};
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_time::{Duration, Instant, Timer};

use super::{KeyNotices, LedWindow};
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

const POLL_INTERVAL_MS: u32 = 4;
const WRITE_MARGIN_MS: u32 = 2;

pub struct Lighting<
    'd,
    M: RawMutex,
    H,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> {
    pub lighting: &'d mut lighting::Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>,
    pub events: &'d KeyEvents,
    pub notices: &'d KeyNotices,
    pub host: &'d HostState,
    pub host_control: &'d HostControl,
    pub diagnostics: &'d Diagnostics,
    pub profile: &'d profile::Shared<M>,
    pub led_window: &'d LedWindow,
}

impl<M, H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Lighting<'_, M, H, ROWS, COLUMNS, PAGES, LENGTH>
where
    M: RawMutex,
    H: Lightbar + Signals,
{
    const FAILED_STEPS_BEFORE_INITIALIZE: u8 = 3;

    pub async fn run(&mut self, matrix: &mut impl LedMatrix, input: &impl KeyTravel) -> ! {
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
                set_running(self.led_window, matrix, false).await;
                initialized = false;
                failed_steps = 0;
            }
            let now = Instant::now();
            select4(
                self.events.ready_to_receive(),
                self.notices.ready(),
                self.host_control.ready(),
                Timer::at(wake_at(now, self.lighting.until_frame_ms(ms_of(now)))),
            )
            .await;
        }
    }

    pub async fn initialize(&mut self, matrix: &mut impl LedMatrix) -> bool {
        let initialized = matrix.initialize().await.is_ok();
        if initialized {
            self.lighting.set_output(false);
        }
        self.diagnostics.set_lighting_initialized(initialized);
        initialized
    }

    pub async fn step(
        &mut self,
        matrix: &mut impl LedMatrix,
        input: &impl KeyTravel,
        now_ms: u32,
        usb_active: bool,
    ) -> bool {
        let lighting = &mut *self.lighting;
        if self.host_control.take_session_end() && H::PRESENT {
            self.profile.host_signals().end_session();
        }
        self.profile.apply(lighting, now_ms);
        while let Ok(activity) = self.events.try_receive() {
            lighting.key_event(activity.key, activity.event, activity.at_ms);
            lighting.hooks_mut().key_event(activity.key, activity.event);
            if activity.event == KeyEvent::Pressed {
                lighting.reset_idle_timer(now_ms);
            }
        }
        self.notices.apply(lighting, now_ms);
        self.host_control.apply(lighting, now_ms);
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
                let running = set_running(self.led_window, matrix, true).await;
                if running {
                    lighting.wake(now_ms);
                    lighting.invalidate();
                } else {
                    lighting.set_output(false);
                }
                running
            }
            Some(OutputChange::ShutDown) => {
                let stopped = set_running(self.led_window, matrix, false).await;
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
        let mut frame_due = frame_deadline(lighting.until_frame_ms(now_ms));
        while let Some(write) = lighting.next_write() {
            let Either::First(transfer) = select(self.led_window.enter(), &mut frame_due).await
            else {
                lighting.requeue_write(&write);
                break;
            };
            let written = matrix
                .write(write.bank, write.first_register, write.values())
                .await;
            drop(transfer);
            self.diagnostics.page_written(&written);
            if written.is_err() {
                lighting.invalidate();
                return false;
            }
            lighting.confirm_write(&write);
        }
        true
    }
}

async fn set_running(window: &LedWindow, matrix: &mut impl LedMatrix, running: bool) -> bool {
    let _transfer = window.enter().await;
    matrix.set_running(running).await.is_ok()
}

fn now_ms() -> u32 {
    ms_of(Instant::now())
}

fn ms_of(instant: Instant) -> u32 {
    u32::try_from(instant.as_millis() & u64::from(u32::MAX)).unwrap_or_default()
}

fn wake_at(now: Instant, until_frame_ms: u32) -> Instant {
    Instant::from_millis(now.as_millis() + u64::from(pause_ms(until_frame_ms)))
}

fn pause_ms(until_frame_ms: u32) -> u32 {
    if until_frame_ms < POLL_INTERVAL_MS {
        until_frame_ms.max(1)
    } else {
        POLL_INTERVAL_MS
    }
}

fn frame_deadline(until_frame_ms: u32) -> Timer {
    if until_frame_ms == 0 || until_frame_ms > lighting::FRAME_PERIOD_MS {
        Timer::at(Instant::MAX)
    } else {
        Timer::after(Duration::from_millis(u64::from(
            until_frame_ms.saturating_sub(WRITE_MARGIN_MS),
        )))
    }
}

fn deepest_travel<const ROWS: usize, const COLUMNS: usize>(input: &impl KeyTravel) -> u16 {
    let mut deepest = 0;
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            if let (Ok(row), Ok(column)) = (u8::try_from(row), u8::try_from(column)) {
                deepest = deepest.max(input.travel(Key::new(row, column)));
            }
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_task_wakes_for_a_frame_that_falls_due_before_the_next_poll() {
        assert_eq!(pause_ms(3), 3);
        assert_eq!(pause_ms(1), 1);
        assert_eq!(pause_ms(0), 1);
        assert_eq!(pause_ms(4), 4);
        assert_eq!(pause_ms(15), 4);
        assert_eq!(pause_ms(u32::MAX), 4);
    }

    #[test]
    fn the_task_wakes_at_the_start_of_the_millisecond_its_frame_falls_due_in() {
        let now = Instant::from_micros(16_700);
        assert_eq!(ms_of(now), 16);
        assert_eq!(wake_at(now, 2), Instant::from_millis(18));
        assert_eq!(wake_at(now, 0), Instant::from_millis(17));
        assert_eq!(wake_at(now, 12), Instant::from_millis(20));
    }
}

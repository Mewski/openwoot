use embassy_futures::select::select3;
use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};

use super::KeyNotices;
use crate::{
    lighting::{self, KeyEvent, KeyEvents, KeyTravel, LedMatrix, Lightbar, OutputChange, Signals},
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
}

impl<M, H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Lighting<'_, M, H, ROWS, COLUMNS, PAGES, LENGTH>
where
    M: RawMutex,
    H: Lightbar + Signals,
{
    const POLL_INTERVAL: Duration = Duration::from_millis(4);

    pub async fn run(&self, matrix: &mut impl LedMatrix, input: &impl KeyTravel) -> ! {
        let mut initialized = matrix.initialize().await.is_ok();
        loop {
            if !initialized {
                initialized = matrix.initialize().await.is_ok();
            }
            self.step(matrix, input, now_ms(), self.host.is_active())
                .await;
            select3(
                self.events.ready_to_receive(),
                self.notices.ready(),
                Timer::after(Self::POLL_INTERVAL),
            )
            .await;
        }
    }

    pub async fn step(
        &self,
        matrix: &mut impl LedMatrix,
        input: &impl KeyTravel,
        now_ms: u32,
        usb_active: bool,
    ) {
        let mut lighting = self.lighting.lock().await;
        while let Ok(activity) = self.events.try_receive() {
            lighting.key_event(activity.key, activity.event, activity.at_ms);
            if activity.event == KeyEvent::Pressed {
                lighting.reset_idle_timer(now_ms);
            }
        }
        self.notices.apply(&mut lighting, now_ms);
        match lighting.set_output(usb_active) {
            Some(OutputChange::Wake) => {
                if matrix.set_running(true).await.is_ok() {
                    lighting.wake(now_ms);
                    lighting.invalidate();
                } else {
                    lighting.set_output(false);
                }
            }
            Some(OutputChange::ShutDown) => {
                let _ = matrix.set_running(false).await;
            }
            None => {}
        }
        if !lighting.output_enabled() {
            return;
        }
        lighting.update(now_ms, input);
        while let Some(write) = lighting.next_write() {
            if matrix
                .write(write.bank, write.first_register, write.values())
                .await
                .is_err()
            {
                lighting.invalidate();
                break;
            }
        }
    }
}

fn now_ms() -> u32 {
    u32::try_from(Instant::now().as_millis() & u64::from(u32::MAX)).unwrap_or_default()
}

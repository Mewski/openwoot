use super::{Call, Key, Notice, Output, record::Record};
use crate::layout::{Layers, Profiles};

pub trait Functions {
    fn call<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        call: Call,
        effects: &mut Effects<'_, ROWS, COLUMNS>,
    ) {
        let _ = (call, effects);
    }
}

pub struct Effects<'a, const ROWS: usize, const COLUMNS: usize> {
    pub(super) layers: &'a mut Layers,
    pub(super) profiles: &'a mut Profiles,
    pub(super) records: &'a mut [[Record; COLUMNS]; ROWS],
    pub(super) output: &'a mut Output,
}

impl Functions for () {}

impl<const ROWS: usize, const COLUMNS: usize> Effects<'_, ROWS, COLUMNS> {
    pub const fn layers(&mut self) -> &mut Layers {
        self.layers
    }

    pub const fn profiles(&mut self) -> &mut Profiles {
        self.profiles
    }

    pub fn ignore_until_release(&mut self, key: Key) {
        if let Some(record) = self.record(key) {
            record.ignore_until_release = true;
        }
    }

    pub fn toggle_disabled(&mut self, key: Key) -> Option<bool> {
        let record = self.record(key)?;
        record.disabled = !record.disabled;
        Some(record.disabled)
    }

    pub fn notify(&mut self, notice: Notice) {
        self.output.notify(notice);
    }

    fn record(&mut self, key: Key) -> Option<&mut Record> {
        self.records
            .get_mut(key.row())
            .and_then(|row| row.get_mut(key.column()))
            .filter(|record| record.present)
    }
}

impl<const ROWS: usize, const COLUMNS: usize> core::fmt::Debug for Effects<'_, ROWS, COLUMNS> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Effects")
            .field("layers", &self.layers)
            .field("profiles", &self.profiles)
            .finish_non_exhaustive()
    }
}

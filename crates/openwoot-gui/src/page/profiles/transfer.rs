use gpui_kit::{AsyncApp, Entity};
use openwoot_protocol::{
    endpoints::{GetProfileInfo, MoveProfile, SetProfileInfo, SwapProfiles},
    profile::{self, Change, Move, Slot, Swap},
};

use super::error_chain;
use crate::{
    profile_parts::{Parts, apply, fetch},
    store::{Scope, Store},
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Snapshot {
    pub(super) info: profile::Info,
    pub(super) parts: Parts,
}

pub(super) const SWAP_STEPS: usize = 2;
pub(super) const REMOVE_STEPS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Onboard {
    pub(super) active: Slot,
    pub(super) count: u8,
    pub(super) lightbar: bool,
}

impl Onboard {
    pub(super) const fn copy_steps(self) -> usize {
        Parts::steps(self.lightbar) + 1
    }

    pub(super) const fn insert_steps(self) -> usize {
        self.copy_steps() + 2
    }

    pub(super) const fn replace_steps(self) -> usize {
        self.copy_steps() + 2
    }
}

pub(super) async fn read(
    store: &Entity<Store>,
    slot: Slot,
    lightbar: bool,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<Snapshot, String> {
    let info = fetch::<GetProfileInfo, profile::Info>(store, slot, cx)
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    let parts = Parts::read(store, slot, lightbar, cx, step)
        .await
        .map_err(|error| error_chain(&error))?;
    Ok(Snapshot { info, parts })
}

pub(super) async fn swap(
    store: &Entity<Store>,
    first: Slot,
    second: Slot,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    let outcome = apply::<SwapProfiles>(store, Swap { first, second }, Scope::Runtime, cx).await;
    forget_history(store, cx);
    outcome.map_err(|error| error_chain(&error))?;
    step(cx);
    reload(store, cx, step).await;
    Ok(())
}

pub(super) async fn remove(
    store: &Entity<Store>,
    slot: Slot,
    onboard: Onboard,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    let last = onboard
        .count
        .checked_sub(1)
        .filter(|last| *last > 0 && slot.get() <= *last && slot.get() > 0)
        .map(Slot::new)
        .ok_or("the keyboard keeps at least one profile")?;
    let outcome = shift_out(store, slot, last, onboard.active, cx, step).await;
    forget_history(store, cx);
    outcome?;
    reload(store, cx, step).await;
    Ok(())
}

async fn shift_out(
    store: &Entity<Store>,
    slot: Slot,
    last: Slot,
    active: Slot,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    if slot != last {
        let shift = Move {
            from: slot,
            to: last,
        };
        apply::<MoveProfile>(store, shift, Scope::Runtime, cx)
            .await
            .map_err(|error| error_chain(&error))?;
    }
    step(cx);
    if active == slot {
        let before = Slot::new(slot.get().saturating_sub(1));
        store
            .update(cx, |store, cx| store.activate(before, cx))
            .await;
    }
    step(cx);
    store
        .update(cx, |store, cx| store.delete_profile(last, cx))
        .await;
    step(cx);
    Ok(())
}

pub(super) async fn insert(
    store: &Entity<Store>,
    snapshot: &Snapshot,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<Slot, String> {
    let slot = store
        .update(cx, |store, cx| store.create_profile(cx))
        .await
        .ok_or("the keyboard did not create a profile")?;
    step(cx);
    write(store, slot, snapshot, cx, step).await?;
    reload(store, cx, step).await;
    Ok(slot)
}

pub(super) async fn replace(
    store: &Entity<Store>,
    slot: Slot,
    snapshot: &Snapshot,
    onboard: Onboard,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    let outcome = write(store, slot, snapshot, cx, step).await;
    forget_history(store, cx);
    outcome?;
    if onboard.active == slot {
        store
            .update(cx, |store, cx| store.save(cx))
            .await
            .map_err(|error| error_chain(&error))?;
    }
    step(cx);
    reload(store, cx, step).await;
    Ok(())
}

async fn write(
    store: &Entity<Store>,
    slot: Slot,
    snapshot: &Snapshot,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    snapshot
        .parts
        .write(store, slot, cx, step)
        .await
        .map_err(|error| error_chain(&error))?;
    let info = Change {
        profile: slot,
        value: snapshot.info.clone(),
    };
    apply::<SetProfileInfo>(store, info, Scope::Profile(slot), cx)
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    Ok(())
}

async fn reload(store: &Entity<Store>, cx: &mut AsyncApp, step: &mut impl FnMut(&mut AsyncApp)) {
    store.update(cx, |store, cx| store.reload(cx)).await;
    step(cx);
}

fn forget_history(store: &Entity<Store>, cx: &mut AsyncApp) {
    store.update(cx, Store::clear_history);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copying_a_profile_covers_the_light_bar_when_there_is_one() {
        let layout = Onboard {
            active: Slot::new(0),
            count: 2,
            lightbar: false,
        };
        let with_lightbar = Onboard {
            lightbar: true,
            ..layout
        };
        assert_eq!(with_lightbar.copy_steps(), layout.copy_steps() + 1);
        assert_eq!(layout.replace_steps(), layout.copy_steps() + 2);
    }
}

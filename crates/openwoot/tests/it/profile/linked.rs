use openwoot::{keys, profile, storage::Settings};
use openwoot_protocol::{
    apps::{self, App, Custom, Filter, Link, Matching},
    configuration::{Invalid, Rejection},
    profile::{Namespace, Selection, Slot},
};

use super::{FACTORY, Mutual, Nor, ProfileModel, storage};

fn open_linked<'a>(
    keys: &'a keys::Shared<Mutual>,
    shared: &'a profile::Shared<Mutual>,
    settings: &'a mut Settings<Nor>,
) -> ProfileModel<'a> {
    let mut model =
        profile::Model::new(keys, shared, Some(settings), &FACTORY, 0, 4).with_linked(2);
    model.load();
    model
}

fn text(value: &str) -> apps::Text {
    apps::Text::try_from(value).unwrap_or_default()
}

fn steam(id: u32) -> Link {
    Link {
        name: text("game"),
        app: App::Steam(id),
    }
}

fn window(title: &str) -> Link {
    Link {
        name: text("editor"),
        app: App::Custom(Custom {
            matching: Matching::All,
            filters: [Filter::WindowTitle(text(title))].into_iter().collect(),
        }),
    }
}

fn link_at(model: &mut ProfileModel<'_>, profile: Slot, index: u8) -> Result<Link, Rejection> {
    let mut link = steam(0);
    model.app_link(profile, index, &mut link).map(|()| link)
}

#[test]
fn linked_profiles_have_their_own_namespace_and_limit() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open_linked(&keys, &shared, &mut settings);
    assert_eq!(model.create(Namespace::Linked), Ok(Slot::linked(0)));
    assert_eq!(model.create(Namespace::Linked), Ok(Slot::linked(1)));
    assert_eq!(
        model.create(Namespace::Linked),
        Err(Rejection::ProfileLimitReached)
    );
    assert_eq!(model.create(Namespace::Onboard), Ok(Slot::new(2)));
    let summary = model.summary();
    assert_eq!((summary.count, summary.linked), (3, 2));
    assert_eq!(
        model.swap(Slot::new(0), Slot::linked(0)),
        Err(Rejection::Invalid(Invalid::ProfileLinked(0x80)))
    );
}

#[test]
fn the_keyboard_starts_on_the_onboard_profile_a_linked_one_was_entered_from() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    {
        let mut model = open_linked(&keys, &shared, &mut settings);
        model.create(Namespace::Linked).unwrap();
        model.activate(Slot::new(1)).unwrap();
        model.activate(Slot::linked(0)).unwrap();
        assert_eq!(model.summary().active, Slot::linked(0));
    }
    let model = open_linked(&keys, &shared, &mut settings);
    let summary = model.summary();
    assert_eq!((summary.active, summary.linked), (Slot::new(1), 1));
}

#[test]
fn a_selection_reaches_the_keys_and_clearing_it_returns_to_the_onboard_profile() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open_linked(&keys, &shared, &mut settings);
    model.create(Namespace::Linked).unwrap();
    model.activate(Slot::new(1)).unwrap();
    let select = |profile, activate| Selection { profile, activate };

    model
        .select_linked(select(Some(Slot::linked(0)), false))
        .unwrap();
    assert_eq!(model.summary().active, Slot::new(1));
    assert_eq!(model.summary().selected, Some(Slot::linked(0)));
    assert_eq!(keys.links(), (Some(0x80), 1));

    model
        .select_linked(select(Some(Slot::linked(0)), true))
        .unwrap();
    assert_eq!(model.summary().active, Slot::linked(0));

    model.select_linked(select(None, false)).unwrap();
    assert_eq!(model.summary().active, Slot::new(1));
    assert_eq!(model.summary().selected, None);
    assert_eq!(
        model.select_linked(select(Some(Slot::new(1)), false)),
        Err(Rejection::Invalid(Invalid::ProfileNotLinked(1)))
    );
}

#[test]
fn deleting_the_active_linked_profile_returns_to_the_onboard_one() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open_linked(&keys, &shared, &mut settings);
    model.create(Namespace::Linked).unwrap();
    model.activate(Slot::new(1)).unwrap();
    model
        .select_linked(Selection {
            profile: Some(Slot::linked(0)),
            activate: true,
        })
        .unwrap();
    model.delete(Slot::linked(0)).unwrap();
    let summary = model.summary();
    assert_eq!(
        (summary.active, summary.linked, summary.selected),
        (Slot::new(1), 0, None)
    );
}

#[test]
fn app_links_append_replace_and_close_the_gap_of_a_deleted_one() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let linked = Slot::linked(0);
    {
        let mut model = open_linked(&keys, &shared, &mut settings);
        model.create(Namespace::Linked).unwrap();
        assert_eq!(model.app_link_count(linked), Ok(0));
        assert_eq!(
            model.set_app_link(linked, 1, &steam(1)),
            Err(Rejection::AppLinkMissing(1))
        );
        for (index, link) in [steam(1), window("a"), steam(3)].iter().enumerate() {
            let index = u8::try_from(index).unwrap();
            model.set_app_link(linked, index, link).unwrap();
        }
        model.set_app_link(linked, 1, &window("b")).unwrap();
        model.delete_app_link(linked, 0).unwrap();
        assert_eq!(
            model.app_link_count(Slot::new(0)),
            Err(Rejection::Invalid(Invalid::ProfileNotLinked(0)))
        );
    }
    let mut model = open_linked(&keys, &shared, &mut settings);
    assert_eq!(model.app_link_count(linked), Ok(2));
    assert_eq!(link_at(&mut model, linked, 0), Ok(window("b")));
    assert_eq!(link_at(&mut model, linked, 1), Ok(steam(3)));
    assert_eq!(
        link_at(&mut model, linked, 2),
        Err(Rejection::AppLinkMissing(2))
    );

    model.delete(linked).unwrap();
    model.create(Namespace::Linked).unwrap();
    assert_eq!(model.app_link_count(linked), Ok(0));
}

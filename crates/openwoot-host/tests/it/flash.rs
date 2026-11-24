use std::path::Path;

use openwoot_host::flash::{self, Error};

#[test]
fn a_missing_image_is_reported_with_its_path() {
    let error = flash::plan(Path::new("missing.hex")).unwrap_err();
    assert!(matches!(error, Error::Read { .. }));
    assert_eq!(error.to_string(), "could not read `missing.hex`");
}

#[test]
fn a_file_without_a_descriptor_is_not_an_image() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    assert!(matches!(
        flash::plan(&manifest),
        Err(Error::NotAnImage { .. })
    ));
}

use openwoot::keys::Index;

#[test]
fn key_index_exists_only_below_the_key_count() {
    assert_eq!(Index::<3>::new(2).map(Index::get), Some(2));
    assert_eq!(Index::<3>::new(3), None);
}

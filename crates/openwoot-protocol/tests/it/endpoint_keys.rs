use openwoot_protocol::endpoints;

#[test]
fn requests_and_responses_of_different_endpoints_never_share_a_key() {
    for keys in [
        endpoints::LIST
            .endpoints
            .iter()
            .map(|(_, request, _)| request.to_bytes())
            .collect::<Vec<_>>(),
        endpoints::LIST
            .endpoints
            .iter()
            .map(|(_, _, response)| response.to_bytes())
            .collect(),
    ] {
        let mut unique = keys.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), keys.len());
    }
}

#[test]
fn the_table_holds_the_openwoot_endpoints_and_the_two_of_postcard_rpc() {
    let mut standard: Vec<_> = endpoints::LIST
        .endpoints
        .iter()
        .map(|(path, _, _)| *path)
        .filter(|path| !path.starts_with("openwoot/"))
        .collect();
    standard.sort_unstable();
    assert_eq!(standard, ["postcard-rpc/ping", "postcard-rpc/schemas/get"]);
    assert_eq!(endpoints::LIST.endpoints.len(), 57);
}

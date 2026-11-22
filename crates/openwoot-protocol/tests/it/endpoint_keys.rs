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

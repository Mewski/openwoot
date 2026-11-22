use postcard_rpc::endpoints;

use crate::Version;

endpoints! {
    list = ENDPOINT_LIST;
    | EndpointTy      | RequestTy | ResponseTy | Path                   |
    | ----------      | --------- | ---------- | ----                   |
    | GetVersion      | ()        | Version    | "openwoot/version"     |
    | EnterUpdateMode | ()        | ()         | "openwoot/update-mode" |
}

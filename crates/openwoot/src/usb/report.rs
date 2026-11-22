use embassy_usb::driver::{EndpointError, EndpointIn};

use crate::hid::InputReport;

pub async fn write_report<Endpoint, Report>(
    endpoint: &mut Endpoint,
    report: &Report,
) -> Result<(), EndpointError>
where
    Endpoint: EndpointIn,
    Report: InputReport,
{
    endpoint
        .write_transfer(report.to_bytes().as_ref(), false)
        .await
}

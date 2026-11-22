use openwoot_image::UsbIds;

const SERIAL_NUMBER_CAPACITY: usize = 49;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity<'a> {
    pub ids: UsbIds,
    pub manufacturer: &'a str,
    pub product: &'a str,
    pub serial_number: Option<&'a str>,
}

#[must_use]
pub fn serial_number_from_page(page: &[u8]) -> Option<&str> {
    let (&len, text) = page.split_first()?;
    let text = core::str::from_utf8(text.get(..usize::from(len))?).ok()?;
    let printable = !text.is_empty()
        && text.len() <= SERIAL_NUMBER_CAPACITY
        && text.bytes().all(|byte| byte.is_ascii_graphic());
    printable.then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_serial_page_holds_a_length_byte_and_the_text() {
        let mut page = [0xFF; 64];
        assert_eq!(serial_number_from_page(&page), None);
        let text = b"A02B2412W031H01234";
        page[0] = 18;
        page[1..19].copy_from_slice(text);
        assert_eq!(serial_number_from_page(&page), Some("A02B2412W031H01234"));
        page[0] = 0;
        assert_eq!(serial_number_from_page(&page), None);
        page[0] = 19;
        assert_eq!(serial_number_from_page(&page), None);
        page[0] = 50;
        page[1..51].fill(b'A');
        assert_eq!(serial_number_from_page(&page), None);
        assert_eq!(serial_number_from_page(&[]), None);
    }
}

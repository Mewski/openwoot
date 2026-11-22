use crate::Error;

pub const MAGIC: [u8; 2] = *b"OW";
pub const SIZES: [usize; 6] = [32, 62, 254, 510, 1022, 2046];
pub const MAX_LEN: usize = ID_LEN + LARGEST;
pub const FRAME_OFFSET: usize = ID_LEN + MAGIC.len() + FRAME_LENGTH_LEN;
pub const MAX_FRAME_LEN: usize = MAX_LEN - FRAME_OFFSET;

const ID_LEN: usize = 1;
const FRAME_LENGTH_LEN: usize = 2;
const LARGEST: usize = match SIZES {
    [.., largest] => largest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contents<'a> {
    Frame(&'a [u8]),
    Foreign { report_id: u8, data: &'a [u8] },
}

pub fn parse(report: &[u8]) -> Result<Contents<'_>, Error> {
    let (&report_id, data) = report.split_first().ok_or(Error::MalformedReport)?;
    match data {
        [first, second, low, high, rest @ ..] if [*first, *second] == MAGIC => {
            let frame_len = usize::from(u16::from_le_bytes([*low, *high]));
            rest.get(..frame_len)
                .map(Contents::Frame)
                .ok_or(Error::MalformedReport)
        }
        _ => Ok(Contents::Foreign { report_id, data }),
    }
}

pub fn seal(report: &mut [u8], frame_len: usize) -> Result<&[u8], Error> {
    let (report_id, size) =
        smallest_holding(FRAME_OFFSET - ID_LEN + frame_len).ok_or(Error::FrameTooLong)?;
    let length = u16::try_from(frame_len).map_err(|_| Error::FrameTooLong)?;
    let report = report.get_mut(..ID_LEN + size).ok_or(Error::FrameTooLong)?;
    let (head, tail) = report
        .split_at_mut_checked(FRAME_OFFSET)
        .ok_or(Error::FrameTooLong)?;
    if let [id, first, second, low, high] = head {
        *id = report_id;
        [*first, *second] = MAGIC;
        [*low, *high] = length.to_le_bytes();
    }
    if let Some(padding) = tail.get_mut(frame_len..) {
        padding.fill(0);
    }
    Ok(report)
}

pub(crate) fn smallest_holding(data_len: usize) -> Option<(u8, usize)> {
    SIZES
        .iter()
        .zip(1..)
        .find(|&(&size, _)| data_len <= size)
        .map(|(&size, report_id)| (report_id, size))
}

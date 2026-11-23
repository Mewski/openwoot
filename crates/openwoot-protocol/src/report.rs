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
    Foreign { report_id: u8, payload: &'a [u8] },
}

pub fn parse(report: &[u8]) -> Result<Contents<'_>, Error> {
    let (&report_id, payload) = report.split_first().ok_or(Error::MalformedReport)?;
    let Some((magic, rest)) = payload.split_first_chunk::<2>() else {
        return Ok(Contents::Foreign { report_id, payload });
    };
    if *magic != MAGIC {
        return Ok(Contents::Foreign { report_id, payload });
    }
    let (length, frame) = rest
        .split_first_chunk::<FRAME_LENGTH_LEN>()
        .ok_or(Error::MalformedReport)?;
    let frame_len = usize::from(u16::from_le_bytes(*length));
    let size = usize::from(report_id)
        .checked_sub(1)
        .and_then(|index| SIZES.get(index))
        .ok_or(Error::MalformedReport)?;
    if FRAME_OFFSET - ID_LEN + frame_len > *size {
        return Err(Error::MalformedReport);
    }
    frame
        .get(..frame_len)
        .map(Contents::Frame)
        .ok_or(Error::MalformedReport)
}

pub fn seal(report: &mut [u8], frame_len: usize) -> Result<&[u8], Error> {
    let (report_id, size) = frame_len
        .checked_add(FRAME_OFFSET - ID_LEN)
        .and_then(smallest_holding)
        .ok_or(Error::FrameTooLong)?;
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

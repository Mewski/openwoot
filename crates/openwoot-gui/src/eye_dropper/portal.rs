use ashpd::desktop::{Color, ResponseError};
use gpui_kit::{App, AppContext as _, Task};
use openwoot_protocol::lighting::Rgb;

const CHANNEL_MAX: f64 = 255.0;

#[derive(Debug, thiserror::Error)]
#[error("could not pick a colour from the screen: {0}")]
pub(crate) struct Error(ashpd::Error);

pub(crate) fn pick(cx: &mut App) -> Task<Result<Option<Rgb>, Error>> {
    cx.background_spawn(async {
        match Color::pick()
            .send()
            .await
            .and_then(|request| request.response())
        {
            Ok(color) => Ok(Some(Rgb {
                red: channel(color.red()),
                green: channel(color.green()),
                blue: channel(color.blue()),
            })),
            Err(ashpd::Error::Response(ResponseError::Cancelled)) => Ok(None),
            Err(error) => Err(Error(error)),
        }
    })
}

fn channel(level: f64) -> u8 {
    let scaled = (level.clamp(0.0, 1.0) * CHANNEL_MAX).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is clamped to 0..=255 and rounded"
    )]
    let channel = scaled as u8;
    channel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_levels_round_to_channels() {
        assert_eq!(channel(0.0), 0);
        assert_eq!(channel(0.5), 128);
        assert_eq!(channel(1.0), 255);
        assert_eq!(channel(-0.25), 0);
        assert_eq!(channel(1.5), 255);
    }
}

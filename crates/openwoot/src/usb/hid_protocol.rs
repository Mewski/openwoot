#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HidProtocol {
    Boot,
    #[default]
    Report,
}

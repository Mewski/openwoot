use assert_cmd::Command;

pub(crate) fn openwoot() -> Command {
    Command::new(env!("CARGO_BIN_EXE_openwoot"))
}

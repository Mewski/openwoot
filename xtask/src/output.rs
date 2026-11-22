use std::fmt::Display;

pub(crate) fn show(label: &str, value: impl Display, width: usize) {
    println!("{:<width$} {value}", format!("{label}:"));
}

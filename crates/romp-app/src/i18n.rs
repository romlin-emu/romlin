/// English labels shared by the existing UI callers.
pub fn translate(english: &str) -> String {
    english.to_owned()
}

pub const fn rtl() -> bool {
    false
}

#[macro_export]
macro_rules! gettext_noop {
    ($s:literal) => {
        $s
    };
}

#[macro_export]
macro_rules! print_mclog {
        ($fmt:expr) => (print!(concat!("[", "{}{}", "]",$fmt), "MINE".green().bold(), "CRAFT".truecolor(122, 82, 49).bold()));
        ($fmt:expr, $($arg:tt)*) => (print!(concat!("[", "{}{}", "]", $fmt), "MINE".green().bold(), "CRAFT".truecolor(122, 82, 49).bold(), $($arg)*));
}

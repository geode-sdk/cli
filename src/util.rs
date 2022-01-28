#[macro_export]
macro_rules! print_error {
    ($x:expr $(, $more:expr)*) => {{
        println!("{}", format!($x, $($more),*).red());
        ::std::process::exit(1);
    }}
}
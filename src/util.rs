#[macro_export]
macro_rules! throw_error {
    ($x:expr $(, $more:expr)*) => {{
        return Err(Box::new(::std::io::Error::new(::std::io::ErrorKind::Other,
            format!($x, $($more),*)
        )));
    }}
}

#[macro_export]
macro_rules! throw_unwrap {
    ($x:expr, $str:expr) => {{
        match $x {
            Ok(a) => a,
            Err(_) => throw_error!($str)
        }
    }}
}
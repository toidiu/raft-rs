macro_rules! cast_unsafe {
    ($target: expr, $pat: path) => {{
        if let $pat(inner) = $target {
            inner
        } else {
            panic!("mismatch variant when cast to {}", stringify!($pat));
        }
    }};
}

pub(crate) use cast_unsafe;

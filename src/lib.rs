#![allow(dead_code)]

// The API has not been decided upon so gate all public usage behind the `testing` feature flag.
macro_rules! testing_visible {
    ($($name:ident);* $(;)?) => {
        $(
            #[cfg(feature = "testing")]
            pub mod $name;
            #[cfg(not(feature = "testing"))]
            mod $name;
        )*
    };
}

testing_visible! {
    mode;
    packet;
    queue;
    server;
    state;
    timeout;
}

mod error;
mod macros;

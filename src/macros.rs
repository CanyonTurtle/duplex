#[macro_export]
macro_rules! console {
    ( $x:expr, $( $y:expr ),* ) => {
        #[cfg(target_arch = "wasm32")]
        crate::web::interop::console_log(&format!($x, $($y),*), 0);
        #[cfg(not(target_arch = "wasm32"))]
        println!($x, $($y),*);
    };
    ( $x:expr ) => {
        #[cfg(target_arch = "wasm32")]
        crate::web::interop::console_log(&format!($x), 0);
        #[cfg(not(target_arch = "wasm32"))]
        println!($x);
    };
}

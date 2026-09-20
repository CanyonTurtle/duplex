#[macro_use]
mod macros;
#[cfg(target_arch = "wasm32")]
#[macro_use]
mod web;
mod parser;
mod cube;
mod alg;
mod enumerate;
#[cfg(not(target_arch = "wasm32"))]
mod search;
#[cfg(not(target_arch = "wasm32"))]
mod candidates;
#[cfg(not(target_arch = "wasm32"))]
mod optimize;
#[cfg(not(target_arch = "wasm32"))]
mod cli;

#[cfg(target_arch = "wasm32")]
fn main() {
     console!("wasm says hi");
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    cli::run();
}

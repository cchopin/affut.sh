#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    affut::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}

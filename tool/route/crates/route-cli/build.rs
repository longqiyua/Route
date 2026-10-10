fn main() {
    // Increase the stack size for the debug binary.
    // The `Commands` enum is large enough that clap's derive-macro-generated
    // help-text builder overflows the default 1 MiB Windows debug stack.
    println!("cargo:rustc-link-arg=/STACK:8388608");
}

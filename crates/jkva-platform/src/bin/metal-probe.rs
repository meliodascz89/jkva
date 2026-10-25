//! Prints the Metal device present on this machine. `cargo run -p jkva-platform --bin metal-probe`

fn main() {
    match jkva_platform::metal_device_name() {
        Some(name) => {
            println!("Metal device: {name}");
            println!(
                "Apple9 family: {}",
                if jkva_platform::metal_supports_apple_family() {
                    "yes"
                } else {
                    "no"
                }
            );
        }
        None => {
            eprintln!("Metal is not available on this machine");
            std::process::exit(1);
        }
    }
}

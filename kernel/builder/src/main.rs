use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <kernel-elf> <output-image>", args[0]);
        std::process::exit(1);
    }
    let kernel_path = Path::new(&args[1]);
    let out_path = Path::new(&args[2]);
    let boot = bootloader::BiosBoot::new(kernel_path);
    boot.create_disk_image(out_path).unwrap();
}

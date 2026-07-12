use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kernel_manifest = manifest_dir.join("../../kernel/Cargo.toml");
    let kernel_bin = manifest_dir
        .join("../../kernel/target/x86_64-unknown-none/debug/karnelos-kernel");
    let out_bin = manifest_dir.join(
        "../../kernel/target/x86_64-unknown-none/debug/bootimage-karnelos-kernel.bin",
    );

    // Build the kernel first. We use a nightly explicitly so that
    // `-Zbuild-std` works for both the kernel and the bootloader stages.
    let status = Command::new("cargo")
        .arg("+nightly-2025-07-08")
        .arg("build")
        .arg("--target")
        .arg("x86_64-unknown-none")
        .arg("-Zbuild-std=core,alloc")
        .arg("-Zbuild-std-features=compiler-builtins-mem")
        .arg("--manifest-path")
        .arg(&kernel_manifest)
        .status()
        .expect("failed to invoke cargo to build the kernel");
    assert!(status.success(), "kernel build failed");
    assert!(kernel_bin.exists(), "kernel binary not found after build");

    // Combine the kernel with the bootloader into a bootable BIOS disk image.
    bootloader::BiosBoot::new(&kernel_bin)
        .create_disk_image(&out_bin)
        .expect("failed to create bootable disk image");

    println!("cargo:rerun-if-changed={}", kernel_bin.display());
    println!("cargo:rerun-if-changed={}", kernel_manifest.display());
}

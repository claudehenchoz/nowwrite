fn main() {
    #[cfg(feature = "uefi")]
    {
        println!("cargo:rerun-if-changed=src/");
        println!("cargo:rustc-link-arg-bins=--target=x86_64-unknown-uefi");
    }
    
    #[cfg(feature = "linux")]
    {
        println!("cargo:rerun-if-changed=src/");
    }
}
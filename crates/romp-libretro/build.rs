fn main() {
    cc::Build::new().file("src/c_log.c").compile("romlin_c_log");
    println!("cargo:rerun-if-changed=src/c_log.c");
}

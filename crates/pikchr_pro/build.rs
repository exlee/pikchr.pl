fn main() {
    cc::Build::new()
        .file("native/pikchr/pikchr.c")
        .compile("pikchr");

    println!("cargo:rerun-if-changed=native/pikchr/pikchr.c");
}

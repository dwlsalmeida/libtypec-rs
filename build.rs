fn main() {
    #[cfg(feature = "c_api")]
    {
        // Find out whether we're in debug or release mode.
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let profile = std::env::var("PROFILE").unwrap();
        let target_dir = std::path::Path::new("target").join(profile);

        run_cbindgen(&out_dir, &target_dir);
        build_c_examples(&target_dir);
        generate_pkg_config(&out_dir, &target_dir);
    }
}

#[cfg(feature = "c_api")]
fn run_cbindgen(out_dir: &String, target_dir: &std::path::Path) {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let config = cbindgen::Config::from_file("cbindgen.toml")
        .unwrap_or_else(|_| cbindgen::Config::default());

    let header_path = std::path::Path::new(&out_dir).join("libtypec-rs.h");
    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .with_language(cbindgen::Language::C)
        .with_parse_expand(&["libtypec-rs"])
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(&header_path);

    std::fs::copy(&header_path, target_dir.join("libtypec-rs.h")).unwrap();
}

#[cfg(feature = "c_api")]
fn build_c_examples(target_dir: &std::path::Path) {
    cc::Build::new()
        .file("examples/c/lstypec.c")
        .include(&target_dir)
        .compile("c_examples_lstypec");

    println!("cargo::rerun-if-changed=examples/c/lstypec.c");
}

#[cfg(feature = "c_api")]
fn generate_pkg_config(out_dir: &String, target_dir: &std::path::Path) {
    use std::io::Write;

    let dest_path = std::path::Path::new(&out_dir).join("libtypec_rs.pc");
    let mut f = std::fs::File::create(&dest_path).unwrap();

    write!(f, "prefix=/usr\n").unwrap();
    write!(f, "exec_prefix=${{prefix}}\n").unwrap();
    write!(f, "libdir=${{exec_prefix}}/lib\n").unwrap();
    write!(f, "includedir=${{prefix}}/include\n").unwrap();
    write!(f, "\n").unwrap();
    write!(f, "Name: libtypec_rs\n").unwrap();
    write!(
        f,
        "Description: USB Type-C Connector System software Interface (UCSI) tools"
    )
    .unwrap();
    write!(f, "Version: 1.0.0\n").unwrap();
    write!(f, "Libs: -L${{libdir}} -ltypec_rs\n").unwrap();
    write!(f, "Cflags: -I${{includedir}}\n").unwrap();

    std::fs::copy(&dest_path, target_dir.join("libtypec_rs.pc")).unwrap();
}

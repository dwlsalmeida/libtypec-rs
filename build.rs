fn main() {
    #[cfg(feature = "c_api")]
    {
        // Find out whether we're in debug or release mode.
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let profile = std::env::var("PROFILE").unwrap();
        let target_dir = std::path::Path::new("target").join(profile);

        run_cbindgen();
        generate_pkg_config(&out_dir, &target_dir);
    }
}

#[cfg(feature = "c_api")]
fn run_cbindgen() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let config = cbindgen::Config::from_file("cbindgen.toml").unwrap();
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("target/include/libtypec-rs.h");
}

#[cfg(feature = "c_api")]
fn generate_pkg_config(out_dir: &String, target_dir: &std::path::Path) {
    use std::io::Write;

    let dest_path = std::path::Path::new(&out_dir).join("libtypec_rs.pc");
    let mut f = std::fs::File::create(&dest_path).unwrap();

    writeln!(f, "prefix=/usr").unwrap();
    writeln!(f, "exec_prefix=${{prefix}}").unwrap();
    writeln!(f, "libdir=${{exec_prefix}}/lib").unwrap();
    writeln!(f, "includedir=${{prefix}}/include").unwrap();
    writeln!(f).unwrap();
    writeln!(f, "Name: libtypec_rs").unwrap();
    writeln!(
        f,
        "Description: USB Type-C Connector System software Interface (UCSI) tools"
    )
    .unwrap();
    writeln!(f, "Version: 1.0.0").unwrap();
    writeln!(f, "Libs: -L${{libdir}} -ltypec_rs").unwrap();
    writeln!(f, "Cflags: -I${{includedir}}").unwrap();

    std::fs::copy(&dest_path, target_dir.join("libtypec_rs.pc")).unwrap();
}

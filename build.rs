use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let c_src_dir = manifest_dir.join("..").join("src");
    let ffi_dir = manifest_dir.join("src").join("ffi");

    fs::create_dir_all(&ffi_dir).expect("create ffi directory");

    build_c_library(&c_src_dir);
    generate_bindings(&c_src_dir, &ffi_dir);
}

fn build_c_library(c_src_dir: &PathBuf) {
    let mut build = cc::Build::new();
    build.include(c_src_dir);
    build.file(c_src_dir.join("zip.c"));
    build.file(c_src_dir.join("miniz.c"));

    if cfg!(target_os = "windows") {
        build.file(c_src_dir.join("winutils.c"));
        build.define("_WIN32", None);
    } else {
        build.file(c_src_dir.join("unixutils.c"));
    }

    // Suppress warnings to avoid compilation errors
    build.flag_if_supported("-w");
    build.flag_if_supported("-Wno-error");
    build.flag_if_supported("-Wno-error=sign-compare");
    build.flag_if_supported("-Wno-error=unused-parameter");
    build.flag_if_supported("-Wno-error=sign-conversion");

    build.compile("zip_c");
    println!("cargo:rustc-link-lib=static=zip_c");

    emit_rerun_if_changed(c_src_dir, "zip.c");
    emit_rerun_if_changed(c_src_dir, "miniz.c");
    emit_rerun_if_changed(c_src_dir, "zip.h");
    emit_rerun_if_changed(c_src_dir, "miniz.h");
    emit_rerun_if_changed(c_src_dir, "unixutils.c");
    emit_rerun_if_changed(c_src_dir, "winutils.c");
}

fn generate_bindings(c_src_dir: &PathBuf, ffi_dir: &PathBuf) {
    let header = c_src_dir.join("zip.h");
    let mut builder = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", c_src_dir.display()))
        .allowlist_function("zip_zip")
        .allowlist_function("zip_unzip")
        .allowlist_function("mz_zip_reader_init_file")
        .allowlist_function("mz_zip_reader_init_cfile")
        .allowlist_function("mz_zip_reader_get_num_files")
        .allowlist_function("mz_zip_reader_file_stat")
        .allowlist_function("mz_zip_reader_locate_file_v2")
        .allowlist_function("mz_zip_reader_extract_to_cfile")
        .allowlist_function("mz_zip_writer_init_file")
        .allowlist_function("mz_zip_writer_init_cfile")
        .allowlist_function("mz_zip_writer_init_from_reader")
        .allowlist_function("mz_zip_reader_end")
        .allowlist_function("mz_zip_writer_end")
        .allowlist_function("mz_zip_writer_add_cfile")
        .allowlist_function("mz_zip_writer_finalize_archive")
        .allowlist_function("mz_deflateInit2")
        .allowlist_function("mz_deflate")
        .allowlist_function("mz_deflateEnd")
        .allowlist_function("mz_deflateBound")
        .allowlist_function("mz_inflateInit2")
        .allowlist_function("mz_inflate")
        .allowlist_function("mz_inflateEnd")
        .allowlist_function("mz_crc32")
        .allowlist_function("zip_open_utf8")
        .allowlist_function("zip_set_permissions")
        .allowlist_function("zip_file_size")
        .allowlist_function("fclose")
        .allowlist_function("free")
        .allowlist_type("mz_zip_archive_file_stat")
        .allowlist_type("mz_zip_archive")
        .allowlist_type("mz_stream")
        .allowlist_type("mz_ulong")
        .allowlist_type("mz_uint64")
        .allowlist_type("mz_uint32")
        .allowlist_type("mz_bool")
        .allowlist_type("zip_char_t")
        .derive_default(true);

    if cfg!(target_os = "windows") {
        builder = builder.clang_arg("-D_WIN32");
    }

    let bindings = builder.generate().expect("generate bindings");
    let out_path = ffi_dir.join("bindings.rs");
    let mut bindings_content = bindings.to_string();

    // Add zip_char_t type definition
    bindings_content.push_str("\n");
    bindings_content.push_str("#[allow(non_camel_case_types)]\n");
    bindings_content.push_str("pub type zip_char_t = ::std::os::raw::c_char;\n");

    std::fs::write(&out_path, bindings_content).expect("write bindings");
    println!("cargo:rerun-if-changed={}", out_path.display());
}

fn emit_rerun_if_changed(dir: &PathBuf, file: &str) {
    println!("cargo:rerun-if-changed={}", dir.join(file).display());
}

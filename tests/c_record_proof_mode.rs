//! Native EDG regression for avoiding C++ semantic predicates on GNU C records.
//! Requires the pinned native EDG library/environment used by integration CI.
use edgcpp::{compile, Compilation};
use std::path::PathBuf;

fn parse_fixture(filename: &str, language: &[&str]) -> Compilation {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("fixtures").join(filename);
    assert!(path.is_file(), "missing test fixture: {}", path.display());
    let mut args = vec![String::from("edgcpfe")];
    args.extend(language.iter().map(|arg| String::from(*arg)));
    args.push(path.to_string_lossy().into_owned());
    let result = compile(&args).expect("native EDG bridge must produce a well-formed snapshot");
    assert_eq!(result.status, 0, "EDG must accept the header-free fixture");
    result
}

#[test]
fn c_records_never_claim_cpp_class_or_lifetime_proofs() {
    let result = parse_fixture(
        "c_mode_records.c",
        &["--gcc", "--c11", "--no_strict_gnu", "--gnu_version", "150300"],
    );
    assert_eq!(result.il.cpp_language, Some(false));
    assert!(!result.il.types.is_empty(), "C aggregate types must still be exported");
    assert!(
        result.il.cxx_record_admissions.iter().all(|r| !r.plain_object),
        "C records must not be certified as C++ POD/trivial classes"
    );
    assert!(
        result.il.cxx_tagless_storage_proofs.is_empty(),
        "C records must not receive C++ tagless storage certificates"
    );
    assert!(
        result.il.cxx_scalar_lifetime_admissions.iter().all(|r| !r.candidate),
        "C records must not acquire C++ destructor/constructor certificates"
    );
}

#[test]
fn cpp_records_still_parse_with_cpp_language_provenance() {
    let result = parse_fixture(
        "cxx_mode_records.cpp",
        &["--g++", "--c++17", "--no_strict_gnu", "--gnu_version", "150300"],
    );
    assert_eq!(result.il.cpp_language, Some(true));
    assert!(!result.il.types.is_empty());
}

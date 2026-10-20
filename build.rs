use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=cpp/bridge.cpp");
    println!("cargo:rerun-if-changed=tools/build_bridge.py");
    for key in ["EDGCPP_COMPILE_COMMANDS", "EDGCPP_LIB_DIR", "EDGCPP_SOURCE_DIR"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let commands = env::var("EDGCPP_COMPILE_COMMANDS")
        .expect("Set EDGCPP_COMPILE_COMMANDS to the EDG CMake compile_commands.json");
    let libdir = env::var("EDGCPP_LIB_DIR")
        .expect("Set EDGCPP_LIB_DIR to the directory containing libedgcpfe.a");
    let source = env::var("EDGCPP_SOURCE_DIR")
        .expect("Set EDGCPP_SOURCE_DIR to the matching EDG checkout");
    // The native EDG IL layout changes when upstream sources or macro
    // configuration change. CI passes an immutable EDG source revision and
    // keys the target cache by that revision, avoiding needless rebuilds
    // caused by fresh checkout timestamps. Local builds without that
    // revision explicitly track native inputs for correctness.
    println!("cargo:rerun-if-env-changed=EDGCPP_EDG_REVISION");
    if env::var_os("EDGCPP_EDG_REVISION").is_none() {
        println!("cargo:rerun-if-changed={commands}");
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(&libdir).join("libedgcpfe.a").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(&source).join("src").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(&source).join("build/gcc-release/src/includes").display()
        );
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let status = Command::new("python3")
        .arg("tools/build_bridge.py")
        .arg(&commands)
        .arg(&source)
        .arg("cpp/bridge.cpp")
        .arg(out.join("bridge.o"))
        .status()
        .expect("Python 3 required to build EDG C++ bridge");
    assert!(status.success(), "EDG C++ bridge compile failed");
    let status = Command::new("ar")
        .arg("crs")
        .arg(out.join("libedgcpp_bridge.a"))
        .arg(out.join("bridge.o"))
        .status()
        .expect("ar required for the EDG C++ bridge");
    assert!(status.success(), "EDG C++ bridge archiving failed");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-search=native={libdir}");
    println!("cargo:rustc-link-lib=static=edgcpp_bridge");
    println!("cargo:rustc-link-lib=static=edgcpfe");
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=m");
}

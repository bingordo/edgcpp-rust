//! End-to-end native executable smoke test for the optional reference backend.
#![cfg(all(feature = "codegen", target_os = "linux", target_arch = "x86_64"))]
use edgcpp::codegen::{emit_object, Expr, Function};
use std::{fs, process::Command};

#[test]
fn cranelift_object_links_and_runs() {
    let object = emit_object(&[Function {
        name: "edg_reference".into(),
        parameters: 2,
        result: Expr::Sub(
            Box::new(Expr::Mul(
                Box::new(Expr::Parameter(0)),
                Box::new(Expr::Constant(7)),
            )),
            Box::new(Expr::Parameter(1)),
        ),
    }]).expect("generate ELF");
    let unique = format!("edgcpp-codegen-{}-{:?}", std::process::id(), std::thread::current().id());
    let dir = std::env::temp_dir().join(unique);
    fs::create_dir_all(&dir).unwrap();
    let result = (|| {
        let obj = dir.join("test.o");
        let source = dir.join("main.c");
        let executable = dir.join("test");
        fs::write(&obj, object).unwrap();
        fs::write(&source, r#"
            #include <stdint.h>
            extern int64_t edg_reference(int64_t a, int64_t b);
            int main(void) {
                return edg_reference(9, 5) == 58 &&
                       edg_reference(-2, 3) == -17 ? 0 : 1;
            }
        "#).unwrap();
        let status = Command::new("cc").arg(&source).arg(&obj)
            .arg("-o").arg(&executable).status().expect("cc installed");
        assert!(status.success(), "link failed");
        let status = Command::new(&executable).status().expect("run executable");
        assert!(status.success(), "generated machine code returned incorrect values");
    })();
    let _ = fs::remove_dir_all(dir);
    result
}

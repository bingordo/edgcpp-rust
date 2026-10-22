//! Small standalone Cranelift ELF emitter, intentionally independent of EDG's
//! native enum discriminants. EDG lowering will feed this backend after decoding
//! the IL; it must never silently guess a native expression operator.
use cranelift_codegen::ir::{types, AbiParam, InstBuilder};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

/// An explicitly typed integer-only reference program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub parameters: usize,
    pub result: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Constant(i64),
    Parameter(usize),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
pub enum CodegenError {
    UnsupportedTarget(String),
    InvalidFunction(String),
    Backend(String),
}

fn emit_expr(
    expr: &Expr,
    builder: &mut FunctionBuilder<'_>,
    params: &[cranelift_codegen::ir::Value],
) -> Result<cranelift_codegen::ir::Value, CodegenError> {
    Ok(match expr {
        Expr::Constant(value) => builder.ins().iconst(types::I64, *value),
        Expr::Parameter(index) => *params.get(*index).ok_or_else(|| {
            CodegenError::InvalidFunction(format!("parameter {index} out of range"))
        })?,
        Expr::Add(a, b) => {
            let a = emit_expr(a, builder, params)?;
            let b = emit_expr(b, builder, params)?;
            builder.ins().iadd(a, b)
        }
        Expr::Sub(a, b) => {
            let a = emit_expr(a, builder, params)?;
            let b = emit_expr(b, builder, params)?;
            builder.ins().isub(a, b)
        }
        Expr::Mul(a, b) => {
            let a = emit_expr(a, builder, params)?;
            let b = emit_expr(b, builder, params)?;
            builder.ins().imul(a, b)
        }
    })
}

/// Emit relocatable ELF64 x86-64 code for a set of simple i64 functions.
/// All functions have the SysV C ABI and return one i64.
pub fn emit_object(functions: &[Function]) -> Result<Vec<u8>, CodegenError> {
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").map_err(|e| CodegenError::Backend(e.to_string()))?;
    let isa_builder = cranelift_codegen::isa::lookup_by_name("x86_64")
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?;
    let isa = isa_builder.finish(settings::Flags::new(flags))
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?;
    let builder = ObjectBuilder::new(isa, "edgcpp", cranelift_module::default_libcall_names())
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    let mut module = ObjectModule::new(builder);
    for function in functions {
        if function.name.is_empty() || function.name.as_bytes().contains(&0) {
            return Err(CodegenError::InvalidFunction("invalid function name".into()));
        }
        let mut context = module.make_context();
        for _ in 0..function.parameters {
            context.func.signature.params.push(AbiParam::new(types::I64));
        }
        context.func.signature.returns.push(AbiParam::new(types::I64));
        let id = module.declare_function(&function.name, Linkage::Export, &context.func.signature)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        let mut fb_context = FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut context.func, &mut fb_context);
            let entry = fb.create_block();
            fb.append_block_params_for_function_params(entry);
            fb.switch_to_block(entry);
            fb.seal_block(entry);
            let params = fb.block_params(entry).to_vec();
            let result = emit_expr(&function.result, &mut fb, &params)?;
            fb.ins().return_(&[result]);
            fb.finalize();
        }
        module.define_function(id, &mut context)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        module.clear_context(&mut context);
    }
    let object = module.finish();
    object.emit().map_err(|e| CodegenError::Backend(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_elf_object() {
        let bytes = emit_object(&[Function {
            name: "add_two".into(),
            parameters: 2,
            result: Expr::Add(Box::new(Expr::Parameter(0)), Box::new(Expr::Parameter(1))),
        }]).unwrap();
        assert_eq!(&bytes[..4], b"\\x7fELF");
        assert_eq!(bytes[4], 2); // ELFCLASS64
    }
    #[test]
    fn rejects_invalid_parameter() {
        assert!(matches!(emit_object(&[Function {
            name: "bad".into(), parameters: 0, result: Expr::Parameter(0)
        }]), Err(CodegenError::InvalidFunction(_))));
    }
}

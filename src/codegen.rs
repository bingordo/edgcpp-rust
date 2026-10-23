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


use crate::{Il, graph::{Graph, ExpressionId}};

extern "C" {
    fn edgcpp_codegen_kind(category: u32, value: u32) -> u32;
}
fn semantic(category: u32, value: u32) -> u32 {
    unsafe { edgcpp_codegen_kind(category, value) }
}

/// Decode a conservative, side-effect-free EDG expression into the reference IR.
/// Unsupported nodes are errors, never silently replaced with zero.
fn lower_expression(
    graph: &Graph,
    id: ExpressionId,
    parameters: &[crate::SymbolId],
) -> Result<Expr, CodegenError> {
    let node = graph.expression(id).ok_or_else(|| CodegenError::InvalidFunction("missing expression".into()))?;
    let data = &node.data;
    match semantic(2, data.kind) {
        1 => data.constant_integer.map(Expr::Constant)
            .ok_or_else(|| CodegenError::InvalidFunction("unsupported constant".into())),
        2 => {
            let symbol = data.symbol_id.ok_or_else(|| CodegenError::InvalidFunction("unresolved variable".into()))?;
            let index = parameters.iter().position(|p| *p == symbol)
                .ok_or_else(|| CodegenError::InvalidFunction("non-parameter variable".into()))?;
            Ok(Expr::Parameter(index))
        }
        3 => {
            if node.operands.len() != 2 {
                return Err(CodegenError::InvalidFunction("expected binary expression".into()));
            }
            let operator = data.operator.map(|op| semantic(3, op)).unwrap_or(0);
            if !(1..=3).contains(&operator) {
                return Err(CodegenError::InvalidFunction("unsupported operator".into()));
            }
            let left = Box::new(lower_expression(graph, node.operands[0], parameters)?);
            let right = Box::new(lower_expression(graph, node.operands[1], parameters)?);
            Ok(match operator {
                1 => Expr::Add(left, right),
                2 => Expr::Sub(left, right),
                3 => Expr::Mul(left, right),
                _ => unreachable!(),
            })
        }
        _ => Err(CodegenError::InvalidFunction(format!("unsupported EDG expression kind {}", data.kind))),
    }
}

/// Lower functions consisting of a single return expression, optionally nested
/// in a block. Other functions are rejected rather than miscompiled.
pub fn lower_edg(il: &Il) -> Result<Vec<Function>, CodegenError> {
    let graph = Graph::from_il(il)
        .map_err(|e| CodegenError::InvalidFunction(format!("invalid EDG graph: {e:?}")))?;
    let mut output = Vec::new();
    for entity in &il.entities {
        let crate::Entity::Routine { name, has_definition: true } = entity else { continue };
        let mut returns = Vec::new();
        let mut invalid = false;
        graph.walk_statements(name, |_, node| {
            match semantic(1, node.data.kind) {
                1 => returns.push(node.expression),
                2 => {}
                _ => invalid = true,
            }
            true
        });
        if invalid || returns.len() != 1 {
            return Err(CodegenError::InvalidFunction(format!(
                "{name}: only one return in a block is supported"
            )));
        }
        let expression = returns[0].ok_or_else(|| CodegenError::InvalidFunction(format!(
            "{name}: return has no expression"
        )))?;
        let routine = graph.routine_for_function(name).ok_or_else(|| CodegenError::InvalidFunction(format!(
            "{name}: missing routine identity"
        )))?;
        let mut parameters: Vec<_> = il.function_parameters.iter()
            .filter(|p| p.routine == routine && !p.implicit_this)
            .collect();
        parameters.sort_by_key(|p| p.position);
        if parameters.iter().enumerate().any(|(i, p)| p.position != Some(i as u32)) {
            return Err(CodegenError::InvalidFunction(format!("{name}: invalid parameter positions")));
        }
        let ids: Vec<_> = parameters.iter().map(|p| p.symbol).collect();
        let result = lower_expression(&graph, expression, &ids)?;
        output.push(Function { name: name.clone(), parameters: ids.len(), result });
    }
    Ok(output)
}

/// Translate an owned EDG compilation snapshot directly into a relocatable ELF.
pub fn emit_edg_object(il: &Il) -> Result<Vec<u8>, CodegenError> {
    emit_object(&lower_edg(il)?)
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
        assert_eq!(&bytes[..4], b"\x7fELF");
        assert_eq!(bytes[4], 2); // ELFCLASS64
    }
    #[test]
    fn rejects_invalid_parameter() {
        assert!(matches!(emit_object(&[Function {
            name: "bad".into(), parameters: 0, result: Expr::Parameter(0)
        }]), Err(CodegenError::InvalidFunction(_))));
    }
}

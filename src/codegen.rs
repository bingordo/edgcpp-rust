//! Small standalone Cranelift ELF emitter, intentionally independent of EDG's
//! native enum discriminants. EDG lowering will feed this backend after decoding
//! the IL; it must never silently guess a native expression operator.
use cranelift_codegen::ir::{types, condcodes::IntCC, AbiParam, InstBuilder};
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
    Neg(Box<Expr>),
    BitAnd(Box<Expr>, Box<Expr>),
    BitOr(Box<Expr>, Box<Expr>),
    BitXor(Box<Expr>, Box<Expr>),
    Compare(CompareOp, Box<Expr>, Box<Expr>),
    Select(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp { Equal, NotEqual, Less, LessEqual, Greater, GreaterEqual }

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
        Expr::Neg(a) => {
            let a = emit_expr(a, builder, params)?;
            builder.ins().ineg(a)
        }
        Expr::BitAnd(a, b) => {
            let a = emit_expr(a, builder, params)?;
            let b = emit_expr(b, builder, params)?;
            builder.ins().band(a, b)
        }
        Expr::BitOr(a, b) => {
            let a = emit_expr(a, builder, params)?;
            let b = emit_expr(b, builder, params)?;
            builder.ins().bor(a, b)
        }
        Expr::BitXor(a, b) => {
            let a = emit_expr(a, builder, params)?;
            let b = emit_expr(b, builder, params)?;
            builder.ins().bxor(a, b)
        }
        Expr::Compare(op, a, b) => {
            let a = emit_expr(a, builder, params)?;
            let b = emit_expr(b, builder, params)?;
            let cc = match op {
                CompareOp::Equal => IntCC::Equal,
                CompareOp::NotEqual => IntCC::NotEqual,
                CompareOp::Less => IntCC::SignedLessThan,
                CompareOp::LessEqual => IntCC::SignedLessThanOrEqual,
                CompareOp::Greater => IntCC::SignedGreaterThan,
                CompareOp::GreaterEqual => IntCC::SignedGreaterThanOrEqual,
            };
            let condition = builder.ins().icmp(cc, a, b);
            let one = builder.ins().iconst(types::I64, 1);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().select(condition, one, zero)
        }
        Expr::Select(condition, yes, no) => {
            let condition = emit_expr(condition, builder, params)?;
            let yes = emit_expr(yes, builder, params)?;
            let no = emit_expr(no, builder, params)?;
            let nonzero = builder.ins().icmp_imm(IntCC::NotEqual, condition, 0);
            builder.ins().select(nonzero, yes, no)
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
            let operator = data.operator.map(|op| semantic(3, op)).unwrap_or(0);
            if matches!(operator, 4 | 11 | 12 | 16 | 17) {
                if node.operands.len() != 1 {
                    return Err(CodegenError::InvalidFunction(format!(
                        "expected unary expression: function={} statement={} path={} operator={:?} operands={}",
                        data.function, data.statement_path, data.path, data.operator, node.operands.len()
                    )));
                }
                let arg = lower_expression(graph, node.operands[0], parameters)?;
                return Ok(match operator {
                    4 => Expr::Neg(Box::new(arg)),
                    11 | 12 => arg,
                    16 => {
                        // A bool-to-integral cast of a comparison only widens
                        // the already canonical 0/1 result. Other casts may
                        // change width or signedness and must be rejected.
                        if !matches!(arg, Expr::Compare(..)) {
                            return Err(CodegenError::InvalidFunction(format!(
                                "unsupported integer cast at {}:{}:{}",
                                data.function, data.statement_path, data.path
                            )));
                        }
                        arg
                    }
                    17 => Expr::Compare(
                        CompareOp::NotEqual, Box::new(arg), Box::new(Expr::Constant(0))
                    ),
                    _ => unreachable!(),
                });
            }
            if node.operands.len() != 2 {
                return Err(CodegenError::InvalidFunction(format!(
                    "expected binary expression: function={} statement={} path={} operator={:?} semantic={} operands={}",
                    data.function, data.statement_path, data.path, data.operator, operator, node.operands.len()
                )));
            }
            if !(1..=3).contains(&operator) && !(5..=10).contains(&operator)
                && !(13..=15).contains(&operator)
            {
                return Err(CodegenError::InvalidFunction(format!(
                    "unsupported EDG operator {:?}", data.operator
                )));
            }
            let left = Box::new(lower_expression(graph, node.operands[0], parameters)?);
            let right = Box::new(lower_expression(graph, node.operands[1], parameters)?);
            Ok(match operator {
                1 => Expr::Add(left, right),
                2 => Expr::Sub(left, right),
                3 => Expr::Mul(left, right),
                5 => Expr::Compare(CompareOp::Equal, left, right),
                6 => Expr::Compare(CompareOp::NotEqual, left, right),
                7 => Expr::Compare(CompareOp::Less, left, right),
                8 => Expr::Compare(CompareOp::LessEqual, left, right),
                9 => Expr::Compare(CompareOp::Greater, left, right),
                10 => Expr::Compare(CompareOp::GreaterEqual, left, right),
                13 => Expr::BitAnd(left, right),
                14 => Expr::BitOr(left, right),
                15 => Expr::BitXor(left, right),
                _ => unreachable!(),
            })
        }
        _ => Err(CodegenError::InvalidFunction(format!("unsupported EDG expression kind {}", data.kind))),
    }
}

/// Lower a structured pure-value control-flow tree. Unsupported statements
/// are rejected (including loops and assignments); no side effects are elided.
fn lower_statement(
    graph: &Graph,
    id: crate::graph::StatementId,
    parameters: &[crate::SymbolId],
) -> Result<Expr, CodegenError> {
    let node = graph.statement(id).ok_or_else(|| {
        CodegenError::InvalidFunction("missing statement".into())
    })?;
    match semantic(1, node.data.kind) {
        1 => {
            if !node.children.is_empty() {
                return Err(CodegenError::InvalidFunction("return has nested statements".into()));
            }
            let expr = node.expression.ok_or_else(|| {
                CodegenError::InvalidFunction("return is missing an expression".into())
            })?;
            lower_expression(graph, expr, parameters)
        }
        2 => {
            // A single statement can be enclosed in arbitrarily many lexical
            // blocks; a sequence of statements is not yet supported.
            if node.expression.is_some() || node.children.len() != 1 {
                return Err(CodegenError::InvalidFunction(
                    "block must contain exactly one executable statement".into()
                ));
            }
            lower_statement(graph, node.children[0], parameters)
        }
        3 => {
            let condition = node.expression.ok_or_else(|| {
                CodegenError::InvalidFunction("if is missing a condition".into())
            })?;
            if node.children.len() != 2 {
                return Err(CodegenError::InvalidFunction(
                    "if must have both returning branches".into()
                ));
            }
            let then_prefix = format!("{}.then.", node.data.path);
            let else_prefix = format!("{}.else.", node.data.path);
            let mut then_stmt = None;
            let mut else_stmt = None;
            for child in &node.children {
                let child_node = graph.statement(*child).ok_or_else(|| {
                    CodegenError::InvalidFunction("missing if child".into())
                })?;
                if child_node.data.path.starts_with(&then_prefix) {
                    if then_stmt.replace(*child).is_some() {
                        return Err(CodegenError::InvalidFunction("duplicate then branch".into()));
                    }
                } else if child_node.data.path.starts_with(&else_prefix) {
                    if else_stmt.replace(*child).is_some() {
                        return Err(CodegenError::InvalidFunction("duplicate else branch".into()));
                    }
                } else {
                    return Err(CodegenError::InvalidFunction("unknown if branch".into()));
                }
            }
            let then_stmt = then_stmt.ok_or_else(|| {
                CodegenError::InvalidFunction("missing then branch".into())
            })?;
            let else_stmt = else_stmt.ok_or_else(|| {
                CodegenError::InvalidFunction("missing else branch".into())
            })?;
            Ok(Expr::Select(
                Box::new(lower_expression(graph, condition, parameters)?),
                Box::new(lower_statement(graph, then_stmt, parameters)?),
                Box::new(lower_statement(graph, else_stmt, parameters)?),
            ))
        }
        _ => Err(CodegenError::InvalidFunction(format!(
            "unsupported EDG statement kind {}", node.data.kind
        ))),
    }
}

/// Resolve any typedef/qualifier wrappers before validating a C ABI type.
/// Cycles, missing definitions and unrelated reference/pointer types are errors.
fn canonical_abi_type(il: &Il, mut ty: crate::TypeId) -> Result<crate::TypeId, CodegenError> {
    for _ in 0..=il.types.len() {
        let definition = il.types.iter().find(|t| t.id == ty).ok_or_else(|| {
            CodegenError::InvalidFunction(format!("missing EDG type {}", ty.0))
        })?;
        if semantic(4, definition.kind) != 3 {
            return Ok(ty);
        }
        if definition.related_role != Some(4) {
            return Err(CodegenError::InvalidFunction(format!(
                "malformed EDG typeref {}", ty.0
            )));
        }
        ty = definition.related.ok_or_else(|| {
            CodegenError::InvalidFunction(format!("unresolved EDG typeref {}", ty.0))
        })?;
    }
    Err(CodegenError::InvalidFunction("cyclic EDG typeref".into()))
}

/// All accepted functions must have an exact, nonvariadic signed-i64 signature.
/// This is checked before evaluating the body because an otherwise valid body
/// does not prove the machine-level calling convention is correct.
fn validate_routine_abi(
    il: &Il,
    name: &str,
    routine: crate::SymbolId,
    parameters: &[&crate::FunctionParameter],
) -> Result<(), CodegenError> {
    let error = |message: &str| CodegenError::InvalidFunction(format!("{name}: {message}"));
    let function_type = il.symbol_types.iter()
        .find(|s| s.symbol == routine)
        .map(|s| canonical_abi_type(il, s.ty))
        .ok_or_else(|| error("missing declared routine type"))??;
    let definition = il.types.iter().find(|t| t.id == function_type)
        .ok_or_else(|| error("missing function type definition"))?;
    if semantic(4, definition.kind) != 2 || definition.related_role != Some(3) {
        return Err(error("expected EDG routine type with a return type"));
    }
    let signature = il.function_signatures.iter().find(|s| s.owner == function_type)
        .ok_or_else(|| error("missing function signature"))?;
    if !signature.prototyped || signature.variadic {
        return Err(error("unprototyped and variadic functions are unsupported"));
    }
    let returns = canonical_abi_type(il, definition.related
        .ok_or_else(|| error("missing return type"))?)?;
    if !il.signed_i64_types.contains(&returns) {
        return Err(error("return ABI must be signed 64-bit integer"));
    }
    let mut declared: Vec<_> = il.type_parameters.iter()
        .filter(|p| p.owner == function_type).collect();
    declared.sort_by_key(|p| p.position);
    if declared.len() != parameters.len() {
        return Err(error("declared/defined parameter count mismatch"));
    }
    for (i, (decl, param)) in declared.iter().zip(parameters).enumerate() {
        if decl.position != i as u32 || param.position != Some(i as u32) {
            return Err(error("noncontiguous parameter positions"));
        }
        if param.compiler_generated {
            return Err(error("compiler-generated ABI parameter unsupported"));
        }
        let declared_type = canonical_abi_type(il, decl.ty)?;
        let defined_type = canonical_abi_type(il, param.ty)?;
        if declared_type != defined_type {
            return Err(error("declared/defined parameter types disagree"));
        }
        if !il.signed_i64_types.contains(&declared_type) {
            return Err(error("parameter ABI must be signed 64-bit integer"));
        }
    }
    Ok(())
}

/// Require validated signed-i64 signatures and a single structured root.
   /// Unsupported native constructs are rejected before machine code emission.
pub fn lower_edg(il: &Il) -> Result<Vec<Function>, CodegenError> {
    let graph = Graph::from_il(il)
        .map_err(|e| CodegenError::InvalidFunction(format!("invalid EDG graph: {e:?}")))?;
    let mut output = Vec::new();
    for entity in &il.entities {
        let crate::Entity::Routine { name, has_definition: true } = entity else { continue };
        let roots: Vec<_> = graph.roots(name).map(|(id, _)| id).collect();
        if roots.len() != 1 {
            return Err(CodegenError::InvalidFunction(format!(
                "{name}: expected one statement root, found {}", roots.len()
            )));
        }
        let routine = graph.routine_for_function(name).ok_or_else(|| CodegenError::InvalidFunction(format!(
            "{name}: missing routine identity"
        )))?;
        let mut parameters: Vec<_> = il.function_parameters.iter()
            .filter(|p| p.routine == routine && !p.implicit_this)
            .collect();
        if il.function_parameters.iter().any(|p| p.routine == routine && p.implicit_this) {
            return Err(CodegenError::InvalidFunction(format!("{name}: member functions unsupported")));
        }
        parameters.sort_by_key(|p| p.position);
        if parameters.iter().enumerate().any(|(i, p)| p.position != Some(i as u32)) {
            return Err(CodegenError::InvalidFunction(format!("{name}: invalid parameter positions")));
        }
        validate_routine_abi(il, name, routine, &parameters)?;
        let ids: Vec<_> = parameters.iter().map(|p| p.symbol).collect();
        let result = lower_statement(&graph, roots[0], &ids)?;
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

    #[test]
    fn enforces_signed_i64_abi_from_owned_type_metadata() {
        use crate::{FunctionParameter, FunctionSignature, SymbolId, SymbolType,
                    TypeDef, TypeId, TypeParameter};
        let routine_kind = (0..128).find(|&kind| semantic(4, kind) == 2)
            .expect("EDG routine kind mapping");
        let integer_kind = (0..128).find(|&kind| semantic(4, kind) == 1)
            .expect("EDG integer kind mapping");
        let routine = SymbolId(10);
        let signed = TypeId(2);
        let unsigned = TypeId(3);
        let routine_ty = TypeId(1);
        let mut il = Il::default();
        il.types.extend([
            TypeDef { id: routine_ty, kind: routine_kind,
                      related_role: Some(3), related: Some(signed) },
            TypeDef { id: signed, kind: integer_kind, related_role: None, related: None },
            TypeDef { id: unsigned, kind: integer_kind, related_role: None, related: None },
        ]);
        il.signed_i64_types.push(signed);
        il.symbol_types.push(SymbolType { symbol: routine, ty: routine_ty });
        il.function_signatures.push(FunctionSignature {
            owner: routine_ty, prototyped: true, variadic: false,
        });
        il.type_parameters.push(TypeParameter { owner: routine_ty, position: 0, ty: signed });
        let mut parameter = FunctionParameter {
            routine, position: Some(0), symbol: SymbolId(11), ty: signed,
            implicit_this: false, compiler_generated: false,
        };
        assert!(validate_routine_abi(&il, "f", routine, &[&parameter]).is_ok());
        il.types[0].related = Some(unsigned);
        assert!(matches!(validate_routine_abi(&il, "f", routine, &[&parameter]),
            Err(CodegenError::InvalidFunction(_))));
        il.types[0].related = Some(signed);
        il.type_parameters[0].ty = unsigned;
        parameter.ty = unsigned;
        assert!(matches!(validate_routine_abi(&il, "f", routine, &[&parameter]),
            Err(CodegenError::InvalidFunction(_))));
        il.type_parameters[0].ty = signed;
        parameter.ty = signed;
        il.function_signatures[0].variadic = true;
        assert!(matches!(validate_routine_abi(&il, "f", routine, &[&parameter]),
            Err(CodegenError::InvalidFunction(_))));
        il.function_signatures[0].variadic = false;
        assert!(matches!(validate_routine_abi(&il, "f", routine, &[]),
            Err(CodegenError::InvalidFunction(_))));
    }

    #[test]
    fn rejects_cycle_in_typedef_abi() {
        use crate::{TypeDef, TypeId};
        let alias_kind = (0..128).find(|&kind| semantic(4, kind) == 3)
            .expect("EDG typeref kind mapping");
        let mut il = Il::default();
        il.types.push(TypeDef {
            id: TypeId(5), kind: alias_kind,
            related_role: Some(4), related: Some(TypeId(5)),
        });
        assert!(matches!(canonical_abi_type(&il, TypeId(5)),
            Err(CodegenError::InvalidFunction(_))));
    }
}

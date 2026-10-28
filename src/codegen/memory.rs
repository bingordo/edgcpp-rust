//! M4 external-data relocation plumbing. This backend-only probe deliberately
//! avoids the EDG decoder: the same imported-data mechanism can be reused by
//! typed lvalue lowering once global symbol metadata is available.
use super::*;
use super::cfg::normalize_c_value;
use cranelift_codegen::ir::MemFlags;

/// Read a C integer scalar from an address, preserving native width and
/// canonicalizing it to the I64 representation used by the CFG IR.
/// The address must refer to valid, suitably aligned nonvolatile storage.
pub(super) fn load_integer(
    fb: &mut FunctionBuilder<'_>,
    address: cranelift_codegen::ir::Value,
    ty: CAbiInt,
) -> cranelift_codegen::ir::Value {
    let raw = fb.ins().load(ty.cranelift(), MemFlags::new(), address, 0);
    let extended = if ty.bits == 64 { raw }
        else if ty.signed { fb.ins().sextend(types::I64, raw) }
        else { fb.ins().uextend(types::I64, raw) };
    normalize_c_value(fb, extended, ty)
}

/// Store a scalar after C assignment conversion; return the canonical stored
/// value so an assignment expression can use it without reloading memory.
pub(super) fn store_integer(
    fb: &mut FunctionBuilder<'_>,
    address: cranelift_codegen::ir::Value,
    value: cranelift_codegen::ir::Value,
    ty: CAbiInt,
) -> cranelift_codegen::ir::Value {
    let value = normalize_c_value(fb, value, ty);
    let narrow = if ty.bits == 64 { value }
        else { fb.ins().ireduce(ty.cranelift(), value) };
    fb.ins().store(MemFlags::new(), narrow, address, 0);
    value
}

/// Emit an ELF object exporting `m4_increment`. The function increments an
/// externally defined 32-bit C unsigned variable, without owning its storage.
/// The linker resolves the data relocation against the C translation unit.
pub fn external_u32_increment_object(symbol: &str) -> Result<Vec<u8>, CodegenError> {
    if symbol.is_empty() || symbol.as_bytes().contains(&0) {
        return Err(CodegenError::InvalidFunction("invalid external data symbol".into()));
    }
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").map_err(|e| CodegenError::Backend(e.to_string()))?;
    let isa = cranelift_codegen::isa::lookup_by_name("x86_64")
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?
        .finish(settings::Flags::new(flags))
        .map_err(|e| CodegenError::UnsupportedTarget(e.to_string()))?;
    let object_builder = ObjectBuilder::new(isa, "edgcpp-m4", cranelift_module::default_libcall_names())
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    let mut module = ObjectModule::new(object_builder);
    let data_id = module.declare_data(symbol, Linkage::Import, true, false)
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    let signature = module.make_signature();
    let id = module.declare_function("m4_increment", Linkage::Export, &signature)
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    let mut context = module.make_context();
    let global = module.declare_data_in_func(data_id, &mut context.func);
    {
        let mut fb_context = FunctionBuilderContext::new();
        let mut fb = FunctionBuilder::new(&mut context.func, &mut fb_context);
        let entry = fb.create_block();
        fb.switch_to_block(entry);
        fb.seal_block(entry);
        let address = fb.ins().global_value(types::I64, global);
        let ty = CAbiInt { bits: 32, signed: false, is_bool: false };
        let old = load_integer(&mut fb, address, ty);
        let one = fb.ins().iconst(types::I64, 1);
        let next = fb.ins().iadd(old, one);
        store_integer(&mut fb, address, next, ty);
        fb.ins().return_(&[]);
        fb.finalize();
    }
    module.define_function(id, &mut context)
        .map_err(|e| CodegenError::Backend(e.to_string()))?;
    module.finish().emit().map_err(|e| CodegenError::Backend(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_relocatable_external_data_object() {
        let object = external_u32_increment_object("m4_counter").unwrap();
        assert_eq!(&object[..4], b"\x7fELF");
        assert!(object.windows("m4_counter".len()).any(|w| w == b"m4_counter"));
    }
    #[test]
    fn rejects_invalid_external_symbol() {
        assert!(external_u32_increment_object("bad\0symbol").is_err());
        assert!(external_u32_increment_object("").is_err());
    }
    #[test]
    fn imported_global_has_no_owning_definition() {
        let object = external_u32_increment_object("m4_counter").unwrap();
        // The ELF carries the unresolved external's symbol spelling. Its
        // definition belongs to the GCC-compiled translation unit.
        assert!(object.windows(b"m4_increment".len()).any(|w| w == b"m4_increment"));
        assert!(object.windows(b"m4_counter".len()).any(|w| w == b"m4_counter"));
    }
}

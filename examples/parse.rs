use edgcpp::{compile, Entity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example parse -- [EDG options] source.cpp");
        std::process::exit(2);
    }
    let mut edg_args = vec!["edgcpfe".to_owned()];
    edg_args.extend(args.into_iter().skip(1));
    let result = compile(&edg_args).map_err(|e| format!("{e:?}"))?;
    println!("EDG status: {}", result.status);
    if result.status != 0 {
        return Err(format!("EDG reported compilation failure: {}", result.status).into());
    }
    if let Some(schedule) = result.il.file_scope_init_schedule {
        println!("file scope init scheduler available={} complete={} roots={}",
                 schedule.available,schedule.complete,
                 result.il.file_scope_init_routines.len());
        for root in &result.il.file_scope_init_routines {
            let symbol = result.il.symbols.iter().find(|s|s.id==root.routine);
            println!("file scope init root position={} id={} name={:?}",
                     root.position,root.routine.0,
                     symbol.map(|s|s.name.as_str()));
        }
    }
    for startup in &result.il.file_scope_lowered_startup_routines {
        let routine=result.il.symbols.iter().find(|s|s.id==startup.routine);
        println!("file scope lowered startup routine={} priority={} name={:?}",
                 startup.routine.0,startup.priority,
                 routine.map(|s|s.name.as_str()));
    }
    for link in &result.il.file_scope_runtime_links {
        let variable = result.il.symbols.iter().find(|s|s.id==link.link_variable);
        let routine = result.il.symbols.iter().find(|s|s.id==link.startup_routine);
        println!("file scope runtime link variable={} name={:?} startup={} name={:?}",
                 link.link_variable.0, variable.map(|s|s.name.as_str()),
                 link.startup_routine.0, routine.map(|s|s.name.as_str()));
    }
    println!("scope dynamic init entries={}",
             result.il.scope_dynamic_initializers.len());
    for init in &result.il.scope_dynamic_initializers {
        let variable=result.il.symbols.iter().find(|s|Some(s.id)==init.variable);
        let destructor=result.il.symbols.iter().find(|s|Some(s.id)==init.destructor);
        println!("scope dynamic init scope={:?} position={} variable={:?} destructor={:?} kind={}",
                 init.scope,init.position,variable.map(|s|s.name.as_str()),
                 destructor.map(|s|s.name.as_str()),init.kind);
    }
    for attr in &result.il.routine_attributes {
        if attr.compiler_generated {
            let routine = result.il.routine_identities.iter()
                .find(|r|r.routine==attr.routine);
            println!("EDG_GENERATED_ROUTINE symbol={} name={:?} defined={} special={}",
                     attr.routine.0,routine.map(|r|r.function.as_str()),
                     attr.defined,attr.special_kind);
        }
    }
    for base in &result.il.base_classes {
        if let Some(offset)=result.il.base_subobject_offsets.iter().find(|o|
            o.derived==base.derived && o.position==base.position) {
            println!("EDG_BASE_OFFSET derived={} base={} position={} virtual={} offset={:?}",
                     base.derived.0,base.base.0,base.position,base.is_virtual,
                     offset.byte_offset);
        }
    }
    // C++5 std::cout is an imported *data* symbol, not a function.
    // Display raw EDG linkage as evidence only, never invent an ABI name.
    for sym in result.il.symbols.iter().filter(|s|
        s.kind==2 && s.name.contains("cout")) {
        let link=result.il.symbol_link_names.iter().find(|v|v.symbol==sym.id);
        let global=result.il.global_variables.iter().find(|g|g.symbol==sym.id);
        let ty=result.il.symbol_types.iter().find(|t|t.symbol==sym.id);
        println!("EDG_CXX5_DATA_SYMBOL id={} name={:?} raw={:?} mangled={:?} global_linkage={:?} ty={:?}",
                 sym.id.0,sym.name,
                 link.map(|v|v.link_name.as_str()),link.map(|v|v.edg_mangled),
                 global.map(|g|g.linkage),ty.map(|t|t.ty.0));
    }
    // Print the exact overload formal type graph involved in the actual
    // std::cout << const char* call; a C++ reference is not a C pointer.
    for routine in result.il.routine_identities.iter().filter(|r|
        r.function.contains("std::operator<<") &&
        r.function.contains("basic_ostream") &&
        r.function.contains("char const*")).take(8) {
        if let Some(function_ty)=result.il.symbol_types.iter()
            .find(|s|s.symbol==routine.routine).map(|s|s.ty) {
            println!("EDG_CXX5_OSTREAM_OPERATOR routine={} ty={}",
                     routine.routine.0,function_ty.0);
            for parameter in result.il.type_parameters.iter()
                .filter(|p|p.owner==function_ty) {
                let ty=result.il.types.iter().find(|t|t.id==parameter.ty);
                let detail=result.il.type_details.iter()
                    .find(|d|d.owner==parameter.ty);
                println!("  EDG_CXX5_OSTREAM_FORMAL position={} ty={} kind={:?} related={:?} role={:?} reference={:?} rvalue={:?} qualifiers={:?}",
                         parameter.position,parameter.ty.0,ty.map(|t|t.kind),
                         ty.and_then(|t|t.related).map(|t|t.0),
                         ty.and_then(|t|t.related_role),
                         detail.map(|d|d.is_reference),
                         detail.map(|d|d.is_rvalue_reference),
                         detail.and_then(|d|d.qualifiers));
            }
        }
    }
    for entity in &result.il.entities {
        match entity {
            Entity::Routine { name, has_definition } =>
                println!("routine {name}, defined={has_definition}"),
            Entity::Variable { name } => println!("variable {name}"),
            Entity::Type { name } => println!("type {name}"),
            Entity::Namespace { name } => println!("namespace {name}"),
        }
    }
    for class in &result.il.cxx_scalar_lifetime_admissions {
        println!("cxx scalar lifetime owner={} candidate={}",
                 class.owner.0,class.candidate);
    }
    for statement in &result.il.statements {
        println!("statement {} path={} parent={:?} kind={}",
                 statement.function, statement.path, statement.parent, statement.kind);
    }
    for expression in &result.il.expressions {
        println!("expression {} statement={} path={} parent={:?} kind={} op={:?} type={:?} type_id={:?}",
            expression.function, expression.statement_path, expression.path,
            expression.parent, expression.kind, expression.operator, expression.type_kind,
            expression.type_id.map(|id|id.0));
        if let Some(value) = expression.constant_integer {
            println!("integer constant {} = {}", expression.function, value);
        }
        if let Some(reference) = &expression.reference {
            println!("reference {} -> {}", expression.function, reference);
        }
    }
    let graph = edgcpp::graph::Graph::from_il(&result.il)
        .map_err(|e| format!("Invalid EDG IL graph: {e:?}"))?;
    println!("indexed IL statements={} expressions={}",
             graph.statements.len(), graph.expressions.len());
    for source in &graph.statement_sources {
        println!("statement source {} path={} file={} line={} column={} seq={}",
                 source.function, source.path, source.location.file,
                 source.location.line, source.location.column, source.location.sequence);
    }
    for source in &graph.symbol_sources {
        let symbol = graph.symbol(source.symbol).ok_or("missing source symbol")?;
        println!("symbol source id={} name={} file={} line={} column={}",
                 source.symbol.0, symbol.name, source.location.file,
                 source.location.line, source.location.column);
    }
    for source in &graph.expression_sources {
        println!("expression source {} statement={} role={} path={} file={} line={} column={}",
                 source.function, source.statement_path, source.role, source.path,
                 source.location.file, source.location.line, source.location.column);
    }
    for lifetime in &graph.object_lifetimes {
        let routine = graph.symbol(lifetime.routine).ok_or("missing lifetime routine")?;
        println!("object lifetime routine={} id={} parent={:?} kind={}",
                 routine.name, lifetime.id, lifetime.parent, lifetime.kind);
    }
    for link in &graph.scope_lifetimes {
        let routine = graph.symbol(link.routine).ok_or("missing scope lifetime routine")?;
        println!("scope lifetime routine={} scope={} lifetime={}",
                 routine.name, link.scope, link.lifetime);
    }
    for link in &graph.initialization_lifetimes {
        println!("init lifetime function={} statement={} lifetime={:?} destructor={:?}",
                 link.function, link.statement_path, link.lifetime, link.destructor);
    }
    for cleanup in &graph.lifetime_destructions {
        let routine = graph.symbol(cleanup.routine).ok_or("missing cleanup routine")?;
        println!("lifetime destruction routine={} lifetime={} position={} variable={:?} destructor={:?}",
                 routine.name, cleanup.lifetime, cleanup.position,
                 cleanup.variable, cleanup.destructor);
    }
    for label in &graph.statement_labels {
        let target = graph.label_target(label.label);
        println!("statement label {} path={} symbol={} definition={} target={:?}",
                 label.function, label.statement_path, label.label.0,
                 label.definition, target);
    }
    for decl in &graph.statement_declarations {
        let variable = graph.symbol(decl.symbol).ok_or("missing declared symbol")?;
        println!("statement declaration {} path={} position={} name={} symbol={} type={} vla={}",
                 decl.function, decl.statement_path, decl.position,
                 variable.name, decl.symbol.0, decl.ty.0, decl.is_vla);
    }
    for dim in &graph.vla_dimensions {
        println!("VLA dimension function={} statement={} array_type={} expression={} copied={}",
                 dim.function, dim.statement_path, dim.array_type.0,
                 dim.has_expression, dim.copied);
    }
    for init in &graph.statement_initializations {
        let variable = init.symbol.and_then(|id| graph.symbol(id));
        println!("statement initialization {} path={} symbol={:?} name={:?} kind={} role={} follows_execution={} braced={}",
                 init.function, init.statement_path, init.symbol,
                 variable.map(|s| s.name.as_str()), init.kind, init.role,
                 init.follows_execution, init.braced);
    }
    for attrs in &graph.statement_attributes {
        println!("statement attributes {} path={} generated={} lowering={} likely={} unlikely={} fallthrough={}",
                 attrs.function, attrs.statement_path, attrs.compiler_generated,
                 attrs.lowering_generated, attrs.likely, attrs.unlikely, attrs.fallthrough);
    }
    for expr in &graph.control_expressions {
        println!("control expression {} statement={} role={} path={} parent={:?} kind={} op={:?}",
                 expr.function, expr.statement_path, expr.role, expr.path,
                 expr.parent, expr.kind, expr.operator);
    }
    for ty in &result.il.types {
        println!("type id={} kind={} related={:?} role={:?}",
            ty.id.0, ty.kind, ty.related, ty.related_role);
    }
    for field in &result.il.record_fields {
        println!("record field {} offset={} bit={} width={}",
            field.name, field.byte_offset, field.bit_offset, field.bit_width);
    }
    for sig in &result.il.function_signatures {
        println!("function signature id={} prototyped={} variadic={}", sig.owner.0, sig.prototyped, sig.variadic);
    }
    for proof in &result.il.routine_supplements {
        println!("routine supplement id={} extra_info={} parameter_count={}",
            proof.owner.0,proof.has_extra_info,proof.parameter_count);
    }
    for proof in &result.il.native_scalar_proofs {
        println!("native scalar proof owner={} integer={} float={} signed_i64={}",
                 proof.owner.0,proof.integer_abi,proof.standard_float_abi,
                 proof.signed_i64);
    }
    for proof in &result.il.native_type_layout_proofs {
        println!("native type layout owner={} layout={} fixed_bound={}",
                 proof.owner.0,proof.has_layout,proof.has_fixed_array_bound);
    }
    for bound in &result.il.array_bounds {
        println!("native array bound owner={} elements={}",
                 bound.array.0,bound.elements);
    }
    for shape in &result.il.native_record_shapes {
        println!("native record shape owner={} fields={} direct_bases={}",
                 shape.owner.0,shape.field_count,shape.direct_base_count);
    }
    for implicit in &result.il.implicit_pointer_casts {
        println!("implicit pointer cast {} statement={} path={} source_type={}",
            implicit.function,implicit.statement_path,
            implicit.path,implicit.source_type.0);
    }
    for detail in &result.il.type_details {
        println!("type detail id={} reference={} rvalue={} qualifiers={:?}",
            detail.owner.0, detail.is_reference, detail.is_rvalue_reference, detail.qualifiers);
    }
    for expr in &graph.expressions {
        if let Some(id) = expr.data.symbol_id {
            let symbol = graph.symbol(id).ok_or("missing referenced symbol")?;
            println!("symbol reference {} -> {} ({})", id.0, symbol.name, symbol.kind);
        }
    }
    for param in &graph.function_parameters {
        let routine = graph.symbol(param.routine).ok_or("missing parameter routine")?;
        let symbol = graph.symbol(param.symbol).ok_or("missing parameter symbol")?;
        println!("function parameter routine={} position={:?} name={} symbol={} type={} this={} generated={}",
                 routine.name, param.position, symbol.name, param.symbol.0,
                 param.ty.0, param.implicit_this, param.compiler_generated);
    }
    for scope in &graph.local_scopes {
        let routine = graph.symbol(scope.routine).ok_or("missing scope routine")?;
        println!("local scope routine={} id={} parent={:?} kind={} statement={:?}",
                 routine.name, scope.id, scope.parent, scope.kind, scope.statement_path);
    }
    for owner in &graph.static_local_owners {
        let routine = graph.symbol(owner.routine).ok_or("missing static local routine")?;
        let variable = graph.symbol(owner.symbol).ok_or("missing static local symbol")?;
        println!("static local owner routine={} name={} symbol={}",
                 routine.name, variable.name, owner.symbol.0);
    }
    for local in &graph.local_variables {
        let routine = graph.symbol(local.routine).ok_or("missing local routine")?;
        let symbol = graph.symbol(local.symbol).ok_or("missing local symbol")?;
        println!("local variable routine={} scope={} name={} symbol={} type={} static={} generated={}",
                 routine.name, local.scope, symbol.name, local.symbol.0, local.ty.0,
                 local.static_storage, local.compiler_generated);
    }
    for identity in &graph.routine_identities {
        println!("routine identity id={} function={}", identity.routine.0, identity.function);
    }
    for edge in &graph.symbol_types {
        let symbol = graph.symbol(edge.symbol).ok_or("missing typed symbol")?;
        println!("symbol type name={} id={} type={}", symbol.name, edge.symbol.0, edge.ty.0);
    }
    println!("indexed symbols={}", graph.symbols.len());
    for init in &graph.variable_initializers {
        println!("variable initializer id={} kind={} explicit={} uninitialized={}",
                 init.symbol.0, init.kind, init.explicit, init.uninitialized);
    }
    for init in &graph.initializer_integers {
        println!("initializer integer symbol={} value={}", init.symbol.0, init.value);
    }
    for init in &graph.dynamic_initializers {
        println!("dynamic initializer symbol={} kind={} expression={:?} operator={:?}",
                 init.symbol.0, init.kind, init.expression_kind, init.operator);
    }
    for expr in &graph.initializer_expressions {
        println!("initializer expression owner={} path={} parent={:?} kind={} op={:?} type={:?} symbol={:?}",
                 expr.owner.0, expr.path, expr.parent, expr.kind, expr.operator,
                 expr.type_id, expr.symbol_id);
    }
    for base in &graph.base_classes {
        println!("base class derived={} position={} base={} virtual={}",
                 base.derived.0, base.position, base.base.0, base.is_virtual);
    }
    for access in &graph.base_class_access {
        println!("base access derived={} position={} access={:?} pack={}",
                 access.derived.0, access.position, access.access, access.pack_expansion);
    }
    for member in &graph.member_functions {
        let symbol = graph.symbol(member.routine).ok_or("missing member symbol")?;
        println!("member function owner={} name={}", member.owner.0, symbol.name);
    }
    for attributes in &graph.routine_attributes {
        let routine = graph.symbol(attributes.routine).ok_or("missing routine attributes symbol")?;
        println!("routine attributes name={} special={} virtual={} override={} final={} this={} generated={} defined={}",
                 routine.name, attributes.special_kind, attributes.is_virtual,
                 attributes.is_override, attributes.is_final, attributes.has_this_parameter,
                 attributes.compiler_generated, attributes.defined);
    }
    for expr in &graph.constructor_initializer_expressions {
        let routine = graph.symbol(expr.routine).ok_or("missing initializer routine")?;
        println!("constructor expression routine={} position={} path={} parent={:?} kind={} op={:?}",
                 routine.name, expr.position, expr.path, expr.parent, expr.kind, expr.operator);
    }
    for init in &graph.constructor_initializers {
        let symbol = graph.symbol(init.routine).ok_or("missing constructor symbol")?;
        println!("constructor initializer routine={} position={} kind={} base={:?} field={:?}",
                 symbol.name, init.position, init.kind, init.target_type, init.target_field);
    }
    Ok(())
}

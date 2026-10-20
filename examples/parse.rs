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
    for entity in &result.il.entities {
        match entity {
            Entity::Routine { name, has_definition } =>
                println!("routine {name}, defined={has_definition}"),
            Entity::Variable { name } => println!("variable {name}"),
            Entity::Type { name } => println!("type {name}"),
            Entity::Namespace { name } => println!("namespace {name}"),
        }
    }
    for statement in &result.il.statements {
        println!("statement {} path={} parent={:?} kind={}",
                 statement.function, statement.path, statement.parent, statement.kind);
    }
    for expression in &result.il.expressions {
        println!("expression {} statement={} path={} parent={:?} kind={} op={:?} type={:?}",
            expression.function, expression.statement_path, expression.path,
            expression.parent, expression.kind, expression.operator, expression.type_kind);
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
        println!("type id={} kind={} related={:?}", ty.id.0, ty.kind, ty.related);
    }
    for field in &result.il.record_fields {
        println!("record field {} offset={} bit={} width={}",
            field.name, field.byte_offset, field.bit_offset, field.bit_width);
    }
    for sig in &result.il.function_signatures {
        println!("function signature id={} prototyped={} variadic={}", sig.owner.0, sig.prototyped, sig.variadic);
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
        println!("routine attributes name={} special={} virtual={} override={} final={} this={}",
                 routine.name, attributes.special_kind, attributes.is_virtual,
                 attributes.is_override, attributes.is_final, attributes.has_this_parameter);
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

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
    // Event 104 is EDG's original a_variable::init_routine pointer in
    // SEPARATE_ROUTINES_FOR_FILE_SCOPE_DYNAMIC_INITS mode. Do not confuse
    // this source association with a complete lifecycle or executable ABI.
    if !result.il.file_scope_dynamic_init_owners.is_empty() {
        println!("EDG_CXX5_SEPARATE_INIT_OWNERS count={} lifecycle_abi_proven=false",
                 result.il.file_scope_dynamic_init_owners.len());
        for owner in result.il.file_scope_dynamic_init_owners.iter().take(64) {
            let scheduled = result.il.file_scope_init_routines.iter()
                .find(|entry|entry.routine==owner.routine);
            println!("EDG_CXX5_SEPARATE_INIT_OWNER variable={} routine={} scheduled_position={:?} lifecycle_abi_proven=false",
                     owner.variable.0,owner.routine.0,
                     scheduled.map(|entry|entry.position));
        }
    }
    // Validate the native event-104 variable→routine roster against
    // event-67's exact EDG scheduler order. A checked source roster is
    // still NOT executable ctor/dtor/atexit/ELF admission.
    if result.il.file_scope_init_schedule.is_some_and(|s|s.available) {
        match edgcpp::startup_provenance::collect_checked_separate_initializer_schedule(&result.il) {
            Ok(checked) => {
                println!("EDG_CXX5_SEPARATE_CHECKED entries={} lifecycle_abi_proven=false",
                         checked.entries.len());
                for entry in checked.entries.iter().take(64) {
                    println!("EDG_CXX5_SEPARATE_CHECKED_ENTRY ordinal={} variable={} routine={} lifecycle_abi_proven=false",
                             entry.scheduler_position,entry.variable.0,entry.routine.0);
                }
            }
            Err(error) => {
                println!("EDG_CXX5_SEPARATE_CHECKED incomplete={error:?} lifecycle_abi_proven=false");
            }
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
    // EDG's generated startup bodies are often the only remaining source
    // of file-scope initializer execution after post-lowering list draining.
    // Report native immediate call identities, not a complete ELF schedule.
    match edgcpp::startup_provenance::collect_startup_evidence(&result.il) {
        Ok(proof) => {
            println!("EDG_CXX5_STARTUP_EVIDENCE roots={} calls={} executable_schedule_proven=false",
                     proof.roots.len(), proof.immediate_calls.len());
            for root in &proof.roots {
                println!("EDG_CXX5_STARTUP_ROOT_SOURCE routine={} function={:?} source={:?} ordinal={:?} priority={:?} link={:?}",
                         root.routine.0, root.function, root.source, root.ordinal,
                         root.priority, root.link_variable.map(|s|s.0));
            }
            for edge in &proof.immediate_calls {
                let callee_link=edge.callee_symbol.and_then(|id| result.il.symbol_link_names
                    .iter().find(|name|name.symbol==id));
                println!("EDG_CXX5_STARTUP_CALL root={} function={:?} statement={:?} path={:?} form={:?} callee={:?} link={:?} type={:?} diagnostic_only=true",
                         edge.root.0, edge.function, edge.statement_path,
                         edge.expression_path, edge.form,
                         edge.callee_symbol.map(|id|id.0),
                         callee_link.map(|s|s.link_name.as_str()),
                         edge.callee_type.map(|id|id.0));
            }
            // Explore exact native direct calls in *defined local bodies*
            // beyond each root's immediate call sites. This diagnostic does
            // not establish startup order, constructor ownership, destructor
            // registration, cross-TU effects or executable C++ ABI.
            match edgcpp::startup_provenance::trace_startup_calls(&result.il,&proof) {
                Ok(trace) => {
                    println!("EDG_CXX5_STARTUP_TRANSITIVE roots={} calls={} indirect={} external={} revisits={} executable_schedule_proven=false",
                             proof.roots.len(),trace.edges.len(),
                             trace.unresolved_indirect,trace.external_calls,
                             trace.revisited_local_bodies);
                    for edge in trace.edges.iter().take(64) {
                        let link=edge.callee_symbol.and_then(|id|result.il.symbol_link_names
                            .iter().find(|name|name.symbol==id));
                        println!("EDG_CXX5_STARTUP_REACHABLE_CALL root={} caller={} function={:?} statement={:?} path={:?} form={:?} callee={:?} link={:?} local_body={} diagnostic_only=true",
                                 edge.root.0,edge.caller.0,edge.function,
                                 edge.statement_path,edge.expression_path,edge.form,
                                 edge.callee_symbol.map(|id|id.0),
                                 link.map(|name|name.link_name.as_str()),edge.local_body);
                    }
                    if trace.edges.len()>64 {
                        println!("EDG_CXX5_STARTUP_REACHABLE_TRUNCATED count={} output_only=true",
                                 trace.edges.len()-64);
                    }
                }
                Err(error) => {
                    println!("EDG_CXX5_STARTUP_TRANSITIVE incomplete={error:?} executable_schedule_proven=false");
                }
            }
        }
        Err(error) => {
            println!("EDG_CXX5_STARTUP_EVIDENCE incomplete={error:?} executable_schedule_proven=false");
        }
    }
    // Call-local exact source identities remain useful when post-lowering
    // EDG drains the scope initializer list (native event 68). These calls
    // do NOT prove source schedule completeness or executable C++ ABI.
    match edgcpp::startup_provenance::collect_cleanup_call_candidates(&result.il) {
        Ok(candidates) => {
            println!("EDG_CXX5_CLEANUP_CALL_LOCAL candidates={} complete_lifecycle_proven=false",
                     candidates.len());
            for candidate in candidates.iter().take(64) {
                println!("EDG_CXX5_CLEANUP_CALL_LOCAL_CANDIDATE root={} function={:?} statement={:?} call_path={:?} destructor={} object={} dso_handle={} scope_owner_proven=false lifecycle_abi_proven=false",
                         candidate.root.0,candidate.function,candidate.statement_path,
                         candidate.expression_path,candidate.destructor.0,
                         candidate.object.0,candidate.dso_handle.0);
            }
            if candidates.len()>64 {
                println!("EDG_CXX5_CLEANUP_CALL_LOCAL_TRUNCATED output_only_count={}",
                         candidates.len()-64);
            }
        }
        Err(reason) => {
            println!("EDG_CXX5_CLEANUP_CALL_LOCAL incomplete={reason:?} complete_lifecycle_proven=false");
            // Diagnostic only: expose original native expression ownership on
            // a failed source join. This is NOT an alternative acceptance path.
            if let Ok(proof)=edgcpp::startup_provenance::collect_startup_evidence(&result.il) {
                for call in proof.immediate_calls.iter().filter(|call| {
                    call.callee_symbol.is_some_and(|id| result.il.symbol_link_names.iter()
                        .any(|name| name.symbol==id && name.link_name=="__cxa_atexit"))
                }).take(8) {
                    println!("EDG_CXX5_CLEANUP_SOURCE_CALL function={:?} statement={:?} path={:?} callee={:?} type={:?}",
                             call.function,call.statement_path,call.expression_path,
                             call.callee_symbol.map(|id|id.0),call.callee_type.map(|id|id.0));
                    for expr in result.il.expressions.iter().filter(|expr|
                        expr.function==call.function &&
                        expr.statement_path==call.statement_path &&
                        (expr.path==call.expression_path ||
                         expr.path.starts_with(&format!("{}.",call.expression_path)))
                    ).take(24) {
                        println!("EDG_CXX5_CLEANUP_SOURCE_EXPR path={:?} parent={:?} kind={} operator={:?} constant={:?} symbol={:?} type={:?}",
                                 expr.path,expr.parent,expr.kind,expr.operator,expr.constant_kind,
                                 expr.symbol_id.map(|id|id.0),expr.type_id.map(|id|id.0));
                    }
                }
            }
        }
    }
    // Keep every exact call even when EDG has drained its native scope
    // initializer list. UNKNOWN owners are explicit, not guessed ordinals.
    match edgcpp::startup_provenance::collect_partial_cleanup_associations(&result.il) {
        Ok(associations) => {
            let matched=associations.iter().filter(|item|item.scope.is_some()).count();
            println!("EDG_CXX5_CLEANUP_PARTIAL calls={} scoped={} unscoped={} complete_lifecycle_proven=false",
                     associations.len(),matched,associations.len()-matched);
            for item in associations.iter().take(64) {
                println!("EDG_CXX5_CLEANUP_PARTIAL_ENTRY object={} destructor={} scope={:?} ordinal={:?} complete_lifecycle_proven=false",
                         item.call.object.0,item.call.destructor.0,
                         item.scope,item.scope_position);
            }
        }
        Err(error) => {
            println!("EDG_CXX5_CLEANUP_PARTIAL incomplete={error:?} complete_lifecycle_proven=false");
        }
    }
    // The stricter typed event-68 join is *also* diagnostic only.
    match edgcpp::startup_provenance::collect_cleanup_registration_witnesses(&result.il) {
        Ok(witnesses) => {
            println!("EDG_CXX5_CLEANUP_TYPED witnesses={} complete_lifecycle_proven=false",
                     witnesses.len());
            for witness in witnesses.iter().take(64) {
                println!("EDG_CXX5_CLEANUP_TYPED_WITNESS root={} function={:?} statement={:?} call_path={:?} scope={:?} ordinal={} destructor={} object={} dso_handle={} lifecycle_abi_proven=false",
                         witness.root.0,witness.function,witness.statement_path,
                         witness.expression_path,witness.scope,witness.scope_position,
                         witness.destructor.0,witness.object.0,witness.dso_handle.0);
            }
            if witnesses.len()>64 {
                println!("EDG_CXX5_CLEANUP_TYPED_TRUNCATED output_only_count={}",
                         witnesses.len()-64);
            }
        }
        Err(reason) => {
            println!("EDG_CXX5_CLEANUP_TYPED incomplete={reason:?} complete_lifecycle_proven=false");
        }
    }
    // Mandatory native event 100 preserves applied weak/alias/section flags.
    // This is diagnostic only: section bytes and target ELF visibility are
    // not yet exported, so no native object admission follows from it.
    for mat in &result.il.global_materializations {
        let symbol = result.il.symbols.iter().find(|s|s.id == mat.symbol);
        println!("EDG_NATIVE_GLOBAL_ATTR id={} name={:?} weak={} weakref={} alias={} section_present={} diagnostic_only=true",
                 mat.symbol.0,symbol.map(|s|s.name.as_str()),
                 mat.is_weak,mat.is_weakref,mat.is_gnu_alias,
                 mat.has_explicit_section);
        println!("EDG_NATIVE_GLOBAL_ELF id={} name={:?} section={:?} visibility={:?} diagnostic_only=true",
                 mat.symbol.0,symbol.map(|s|s.name.as_str()),
                 mat.section,mat.elf_visibility);
    }
    println!("EDG_CXX5_SCOPE_ASM count={} diagnostic_only=true",
             result.il.native_scope_asm.len());
    for entry in &result.il.native_scope_asm {
        println!("EDG_CXX5_SCOPE_ASM_ENTRY scope={:?} position={} flags={} text={:?} diagnostic_only=true",
                 entry.scope,entry.position,entry.flags,entry.assembly);
    }
    println!("scope dynamic init entries={}",
             result.il.scope_dynamic_initializers.len());
    for init in &result.il.scope_dynamic_initializers {
        let variable=result.il.symbols.iter().find(|s|Some(s.id)==init.variable);
        let destructor=result.il.symbols.iter().find(|s|Some(s.id)==init.destructor);
        let destructor_link=init.destructor.and_then(|id|result.il.symbol_link_names
            .iter().find(|name|name.symbol==id));
        println!("scope dynamic init scope={:?} position={} variable_id={:?} variable={:?} destructor_id={:?} destructor={:?} destructor_link={:?} kind={} shutdown_registration_proven=false",
                 init.scope,init.position,init.variable.map(|id|id.0),
                 variable.map(|s|s.name.as_str()),init.destructor.map(|id|id.0),
                 destructor.map(|s|s.name.as_str()),
                 destructor_link.map(|name|name.link_name.as_str()),init.kind);
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
    // Diagnose which *source-owned* globals and dynamic initializer entries
    // remain after EDG lowering. A missing __ioinit in these exported records
    // does NOT mean that libstdc++ needs no startup effects: it can mean that
    // a candidate was suppressed, not materialized, or not visited.
    let io_symbols: Vec<_> = result.il.symbols.iter().filter(|sym|
        sym.name.contains("__ioinit") || sym.name.contains("_ZStL8__ioinit"))
        .collect();
    let materialized = result.il.global_materializations.iter()
        .filter(|m| !m.is_nonreal && !m.is_prototype_instantiation).count();
    println!("EDG_CXX5_INIT_OWNER_CENSUS globals={} materialized_candidates={} dynamic_entries={} scoped_entries={} ioinit_symbols={} executable_schedule_proven=false",
             result.il.global_variables.len(), materialized,
             result.il.dynamic_initializers.len(),
             result.il.scope_dynamic_initializers.len(), io_symbols.len());
    for mat in result.il.global_materializations.iter().filter(|m|
        !m.is_nonreal && !m.is_prototype_instantiation) {
        let sym=result.il.symbols.iter().find(|s|s.id==mat.symbol);
        let global=result.il.global_variables.iter().find(|g|g.symbol==mat.symbol);
        let init=result.il.variable_initializers.iter().find(|v|v.symbol==mat.symbol);
        let src=result.il.symbol_sources.iter().find(|s|s.symbol==mat.symbol);
        let link=result.il.symbol_link_names.iter().find(|v|v.symbol==mat.symbol);
        println!("EDG_CXX5_MATERIALIZED_GLOBAL id={} name={:?} link={:?} linkage={:?} explicit={:?} init_kind={:?} thread_local={:?} used={} address_taken={} source={:?} line={:?} diagnostic_only=true",
                 mat.symbol.0,sym.map(|s|s.name.as_str()),
                 link.map(|v|v.link_name.as_str()),
                 global.map(|v|v.linkage),global.map(|v|v.explicit_initializer),
                 init.map(|v|v.kind),global.map(|v|v.thread_local),
                 mat.used,mat.address_taken,
                 src.map(|v|v.location.file.as_str()),
                 src.map(|v|v.location.line));
    }
    for sym in &io_symbols {
        let global=result.il.global_variables.iter().find(|v|v.symbol==sym.id);
        let mat=result.il.global_materializations.iter().find(|v|v.symbol==sym.id);
        let init=result.il.variable_initializers.iter().find(|v|v.symbol==sym.id);
        let dynamic=result.il.dynamic_initializers.iter().find(|v|v.symbol==sym.id);
        let link=result.il.symbol_link_names.iter().find(|v|v.symbol==sym.id);
        let source=result.il.symbol_sources.iter().find(|v|v.symbol==sym.id);
        println!("EDG_CXX5_IOINIT_SYMBOL id={} kind={} name={:?} link={:?} linkage={:?} explicit={:?} init_kind={:?} dynamic_kind={:?} nonreal={:?} prototype={:?} template={:?} used={:?} address_taken={:?} source={:?} line={:?} diagnostic_only=true",
                 sym.id.0,sym.kind,sym.name,link.map(|v|v.link_name.as_str()),
                 global.map(|v|v.linkage),global.map(|v|v.explicit_initializer),
                 init.map(|v|v.kind),dynamic.map(|v|v.kind),
                 mat.map(|v|v.is_nonreal),mat.map(|v|v.is_prototype_instantiation),
                 mat.map(|v|v.is_template_variable),
                 mat.map(|v|v.used),mat.map(|v|v.address_taken),
                 source.map(|v|v.location.file.as_str()),
                 source.map(|v|v.location.line));
    }
    for owner in &result.il.dynamic_initializers {
        let sym=result.il.symbols.iter().find(|s|s.id==owner.symbol);
        let mat=result.il.global_materializations.iter().find(|v|v.symbol==owner.symbol);
        let global=result.il.global_variables.iter().find(|v|v.symbol==owner.symbol);
        let source=result.il.symbol_sources.iter().find(|v|v.symbol==owner.symbol);
        println!("EDG_CXX5_DYNAMIC_OWNER id={} name={:?} dynamic_kind={} linkage={:?} nonreal={:?} prototype={:?} template={:?} used={:?} source={:?} line={:?} diagnostic_only=true",
                 owner.symbol.0,sym.map(|s|s.name.as_str()),owner.kind,
                 global.map(|v|v.linkage),
                 mat.map(|v|v.is_nonreal),mat.map(|v|v.is_prototype_instantiation),
                 mat.map(|v|v.is_template_variable),mat.map(|v|v.used),
                 source.map(|v|v.location.file.as_str()),
                 source.map(|v|v.location.line));
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
    // Observe the original native operand trees for a direct call to the
    // Itanium runtime symbol. A matching linker name is not a verified ABI
    // registration, and even three arguments do not establish that a
    // destructor, object, and DSO handle were correctly associated.
    for target in &result.il.call_targets {
        if target.form!=edgcpp::CallTargetForm::DirectRoutine {continue;}
        let Some(callee)=target.callee_symbol else {continue;};
        if !result.il.symbol_link_names.iter().any(|name|
            name.symbol==callee && name.link_name=="__cxa_atexit") {continue;}
        let Some(call)=graph.calls.iter().find(|site|
            site.function==target.function &&
            site.statement_path==target.statement_path &&
            site.path==target.path) else {continue;};
        let Some(args)=graph.call_arguments(call) else {continue;};
        println!("EDG_CXX5_CLEANUP_REGISTRATION_CANDIDATE caller={:?} statement={:?} path={:?} callee={} observed_args={} registered_destructor_proven=false",
                 target.function,target.statement_path,target.path,callee.0,args.len());
        for (position,&root) in args.iter().take(8).enumerate() {
            let mut stack=vec![root];
            let mut seen=std::collections::HashSet::new();
            while let Some(id)=stack.pop() {
                if !seen.insert(id.0) {continue;}
                if seen.len()>64 {
                    println!("EDG_CXX5_CLEANUP_ARGUMENT_TRUNCATED position={} nodes={} diagnostic_only=true",
                             position,seen.len());
                    break;
                }
                let Some(node)=graph.expression(id) else {break;};
                let symbol=node.data.symbol_id;
                let linked=symbol.and_then(|id|result.il.symbol_link_names
                    .iter().find(|name|name.symbol==id));
                let named=symbol.and_then(|id|result.il.symbols
                    .iter().find(|sym|sym.id==id));
                println!("EDG_CXX5_CLEANUP_ARGUMENT position={} root={} path={:?} kind={} op={:?} type={:?} symbol={:?} name={:?} link={:?} reference={:?} constant_kind={:?} integer={:?} diagnostic_only=true",
                         position,id.0,node.data.path,node.data.kind,node.data.operator,
                         node.data.type_id.map(|id|id.0),symbol.map(|id|id.0),
                         named.map(|s|s.name.as_str()),
                         linked.map(|name|name.link_name.as_str()),
                         node.data.reference,node.data.constant_kind,
                         node.data.constant_integer);
                // Independently exported EDG address target events use the
                // same (function,statement,path) as the source expression.
                // A ck_address constant's own event-6 symbol_id is absent.
                // The event-55/48 proof is separate, and it is unsafe to
                // reconstruct an ABI target from the constant's display text.
                for witness in result.il.constant_routine_addresses.iter().filter(|a|
                    a.function==target.function &&
                    a.statement_path==target.statement_path &&
                    a.path==node.data.path) {
                    let owner=result.il.symbols.iter().find(|s|s.id==witness.symbol);
                    let link=result.il.symbol_link_names.iter()
                        .find(|a|a.symbol==witness.symbol);
                    let scope_matches=result.il.scope_dynamic_initializers.iter()
                        .filter(|init|init.destructor==Some(witness.symbol)).count();
                    println!("EDG_CXX5_CLEANUP_ADDRESS position={} path={:?} source=event55 kind=routine target={} name={:?} link={:?} scope_destructor_symbol_matches={} lifecycle_abi_proven=false",
                             position,node.data.path,witness.symbol.0,
                             owner.map(|s|s.name.as_str()),
                             link.map(|s|s.link_name.as_str()),scope_matches);
                }
                for witness in result.il.constant_addresses.iter().filter(|a|
                    a.function==target.function &&
                    a.statement_path==target.statement_path &&
                    a.path==node.data.path) {
                    let owner=result.il.symbols.iter().find(|s|s.id==witness.symbol);
                    let link=result.il.symbol_link_names.iter()
                        .find(|a|a.symbol==witness.symbol);
                    let scope_matches=result.il.scope_dynamic_initializers.iter()
                        .filter(|init|init.variable==Some(witness.symbol)).count();
                    println!("EDG_CXX5_CLEANUP_ADDRESS position={} path={:?} source=event48 kind=variable target={} name={:?} link={:?} byte_offset={} scope_variable_symbol_matches={} lifecycle_abi_proven=false",
                             position,node.data.path,witness.symbol.0,
                             owner.map(|s|s.name.as_str()),
                             link.map(|s|s.link_name.as_str()),
                             witness.byte_offset,scope_matches);
                }
                stack.extend(node.operands.iter().rev().copied());
            }
        }
        if args.len()>8 {
            println!("EDG_CXX5_CLEANUP_ARGUMENT_TRUNCATED remaining={} diagnostic_only=true",
                     args.len()-8);
        }
    }
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
    // EDG logical virtual numbering and class virtual-info placement are
    // source diagnostics, NOT Itanium ABI vtable slots or call permission.
    for info in &result.il.native_virtual_members {
        let routine=result.il.symbols.iter().find(|s|s.id==info.routine)
            .ok_or("missing native virtual member symbol")?;
        println!("EDG_VIRTUAL_MEMBER routine={} name={} owner={} number={:?} info_offset={:?} sharing_base={:?} executable_abi_proven=false",
                 info.routine.0,routine.name,info.owner.0,
                 info.virtual_number,info.virtual_info_offset,
                 info.sharing_base.map(|id|id.0));
    }
    // The normal EDG C-backend interface exposes the lowered call tree,
    // not the original virtual dispatch rewrite or its private node pairs.
    // No synthetic virtual-slot/receiver provenance is reported here.
    // A lowered ordinary eok_call can contain an exact record-field
    // reference to "__vptr" without retaining its pre-lowering virtual
    // routine/receiver identity. Record this *observed tree* only for
    // independently unresolved native call targets. These raw literals
    // must never be used to infer Itanium slots or authorize execution.
    let call_by_site:std::collections::HashMap<(&str,&str,&str),&edgcpp::CallSite>=
        graph.calls.iter().map(|c| (
            (c.function.as_str(),c.statement_path.as_str(),c.path.as_str()),c
        )).collect();
    for target in &result.il.call_targets {
        if target.form!=edgcpp::CallTargetForm::IndirectUnresolved {continue;}
        let key=(target.function.as_str(),target.statement_path.as_str(),
                 target.path.as_str());
        let Some(call)=call_by_site.get(&key) else {continue;};
        let Some(evidence)=graph.callee_vptr_field_reference(call) else {continue;};
        println!("EDG_INDIRECT_CALLEE_VPTR function={} statement={} path={} field={} field_name=__vptr owner={} field_path={} field_byte_offset={} raw_literals={:?} source_virtual_proven=false machine_slot_proven=false executable_abi_proven=false",
                 call.function,call.statement_path,call.path,
                 evidence.field.0,evidence.owner.0,evidence.field_path,
                 evidence.field_byte_offset,evidence.raw_integer_literals);
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

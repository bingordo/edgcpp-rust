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
    for ty in &result.il.types {
        println!("type id={} kind={} related={:?}", ty.id.0, ty.kind, ty.related);
    }
    Ok(())
}

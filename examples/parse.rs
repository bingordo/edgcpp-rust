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
    Ok(())
}

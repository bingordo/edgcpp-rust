//! Check copied native images across the former byte and text caps.
use edgcpp::compile;
fn main() -> Result<(),Box<dyn std::error::Error>> {
    let mut args=vec!["edgcpfe".to_owned()];
    args.extend(std::env::args().skip(1));
    let output=compile(&args).map_err(|e|format!("EDG compilation: {e:?}"))?;
    if output.status!=0 { return Err(format!("EDG status {}",output.status).into()); }
    for (name,size) in [("image_4097",4097),("image_600000",600000)] {
        let id=output.il.symbols.iter().find(|s|s.name==name)
            .ok_or_else(||format!("missing symbol {name}"))?.id;
        let image=output.il.global_aggregate_initializers.iter().find(|i|i.symbol==id)
            .ok_or_else(||format!("missing image {name}"))?;
        assert_eq!(image.bytes.len(),size);
        assert_eq!(image.bytes[0],13);
        assert_eq!(image.bytes[size-1],89);
        assert!(image.bytes[1..size-1].iter().all(|&b|b==0));
        println!("EDG_LARGE_IMAGE {name} bytes={size}");
    }
    let id=output.il.symbols.iter().find(|s|s.name=="audit_many_relocs_513")
        .ok_or("missing pointer array")?.id;
    let image=output.il.global_aggregate_initializers.iter().find(|i|i.symbol==id)
        .ok_or("missing pointer image")?;
    assert_eq!(image.bytes.len(),513*8);
    assert!(image.bytes.iter().all(|&b|b==0));
    let mut offsets=output.il.global_aggregate_relocations.iter()
        .filter(|r|r.symbol==id).map(|r|r.offset).collect::<Vec<_>>();
    offsets.sort_unstable();
    assert_eq!(offsets,(0..513).map(|i|i*8).collect::<Vec<u64>>());
    println!("EDG_LARGE_IMAGE relocations=513");
    Ok(())
}

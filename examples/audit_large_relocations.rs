//! Check native literal and routine relocation identities beyond old ceilings.
use edgcpp::compile;
use std::collections::HashSet;
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut args=vec!["edgcpfe".to_owned()];
    args.extend(std::env::args().skip(1));
    let unit=compile(&args).map_err(|e|format!("{e:?}"))?;
    assert_eq!(unit.status,0);
    let il=&unit.il;
    let symbol=|name:&str|il.symbols.iter().find(|s|s.name==name).unwrap().id;
    let literals=symbol("large_literals");
    let routines=symbol("large_routines");
    let target=symbol("target");
    let n=4101usize;
    let mut ids=HashSet::new();
    let refs:Vec<_>=il.global_literal_relocations.iter().filter(|r|r.symbol==literals).collect();
    assert_eq!(refs.len(),n);
    for (i,r) in refs.iter().enumerate() {
        assert_eq!(r.offset,(i*8) as u64);
        assert_eq!(r.addend,0);
        assert!(ids.insert(r.literal_id));
        let object=&il.global_literal_objects[(r.literal_id-1) as usize];
        assert_eq!(object.id,r.literal_id);
        assert_eq!(object.bytes,format!("literal-{i:05}\0").as_bytes());
    }
    let refs:Vec<_>=il.global_routine_relocations.iter().filter(|r|r.symbol==routines).collect();
    assert_eq!(refs.len(),n);
    for (i,r) in refs.iter().enumerate() {
        assert_eq!(r.offset,(i*8) as u64);
        assert_eq!(r.routine,target);
        assert_eq!(r.addend,0);
    }
    for owner in [literals,routines] {
        let image=il.global_aggregate_initializers.iter().find(|g|g.symbol==owner).unwrap();
        assert_eq!(image.bytes.len(),n*8);
        assert!(image.bytes.iter().all(|b|*b==0));
    }
    println!("EDG_LARGE_RELOCATIONS literals={n} routines={n}");
    Ok(())
}

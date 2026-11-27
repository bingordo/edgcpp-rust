//! Wire identity joins refer to original source witnesses, never byte equality.
use super::*;

fn number(n:u64)->WireField {
    WireField{data:std::ptr::null(),length:0,value:n,tag:wire_tags::UNSIGNED}
}
fn text(bytes:&[u8])->WireField {
    WireField{data:bytes.as_ptr(),length:bytes.len(),value:0,tag:wire_tags::TEXT}
}
fn absent()->WireField {
    WireField{data:std::ptr::null(),length:0,value:0,tag:wire_tags::ABSENT}
}
fn send(state:&mut Collector,kind:u32,fields:&[WireField]) {
    unsafe {collect((state as *mut Collector).cast(),kind,fields.as_ptr(),fields.len(),0);}
}
fn seeded()->Collector {
    let mut state=Collector::default();
    send(&mut state,62,&[text(b"f"),text(b"s"),text(b"0"),number(44),text(b"6100")]);
    send(&mut state,102,&[number(1),number(45),text(b"6100")]);
    send(&mut state,102,&[number(2),number(45),text(b"6100")]);
    assert!(!state.malformed && !state.panicked);
    state
}
fn expression(identity:u64)->[WireField;6] {
    [number(0),text(b"f"),text(b"s"),text(b"0"),number(0),number(identity)]
}
fn static_object(id:u64,identity:u64)->[WireField;6] {
    [number(1),absent(),absent(),absent(),number(id),number(identity)]
}

#[test]
fn joins_original_identity_without_merging_identical_bytes() {
    let mut state=seeded();
    send(&mut state,120,&expression(9));
    send(&mut state,120,&static_object(1,9));
    send(&mut state,120,&static_object(2,10));
    assert!(!state.malformed && !state.panicked);
    assert_eq!(state.il.literal_identities,vec![
        LiteralIdentity::Expression{function:"f".into(),statement_path:"s".into(),path:"0".into(),identity:9},
        LiteralIdentity::Static{literal_id:1,identity:9},
        LiteralIdentity::Static{literal_id:2,identity:10},
    ]);
}

#[test]
fn rejects_missing_duplicate_and_invalid_literal_identity() {
    for bad in [expression(0),static_object(3,9),static_object(1,0)] {
        let mut state=seeded();
        send(&mut state,120,&bad);
        assert!(state.malformed && !state.panicked);
    }
    let mut state=seeded();
    let mut unknown=expression(9);
    unknown[3]=text(b"unknown");
    send(&mut state,120,&unknown);
    assert!(state.malformed);
    let mut state=seeded();
    send(&mut state,120,&expression(9));
    send(&mut state,120,&expression(10));
    assert!(state.malformed);
    let mut state=seeded();
    send(&mut state,120,&static_object(1,9));
    send(&mut state,120,&static_object(2,9));
    assert!(state.malformed);
}
#[test]
fn thousands_of_literal_identities_keep_source_occurrences_disjoint() {
    let mut state=Collector::default();
    for id in 1..=4096_u64 {
        let path=format!("0.{id}");
        send(&mut state,62,&[
            text(b"many_literals"),text(b"0"),text(path.as_bytes()),
            number(44),text(b"6100"),
        ]);
        send(&mut state,120,&[
            number(0),text(b"many_literals"),text(b"0"),
            text(path.as_bytes()),number(0),number(id),
        ]);
        send(&mut state,102,&[number(id),number(45),text(b"6100")]);
        send(&mut state,120,&static_object(id,id));
    }
    assert!(!state.malformed && !state.panicked);
    assert_eq!(state.il.string_literals.len(),4096);
    assert_eq!(state.il.global_literal_objects.len(),4096);
    assert_eq!(state.il.literal_identities.len(),8192);
    // Repeated byte images are not evidence of shared original identity.
    send(&mut state,120,&static_object(1,1));
    assert!(state.malformed);
}

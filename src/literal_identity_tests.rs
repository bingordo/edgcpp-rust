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
    send(&mut state,119,&expression(9));
    send(&mut state,119,&static_object(1,9));
    send(&mut state,119,&static_object(2,10));
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
        send(&mut state,119,&bad);
        assert!(state.malformed && !state.panicked);
    }
    let mut state=seeded();
    let mut unknown=expression(9);
    unknown[3]=text(b"unknown");
    send(&mut state,119,&unknown);
    assert!(state.malformed);
    let mut state=seeded();
    send(&mut state,119,&expression(9));
    send(&mut state,119,&expression(10));
    assert!(state.malformed);
    let mut state=seeded();
    send(&mut state,119,&static_object(1,9));
    send(&mut state,119,&static_object(2,9));
    assert!(state.malformed);
}

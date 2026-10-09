use super::*;

mod common_compose_candidate {
    use super::*;
    fn compose_value()->serde_json::Value {serde_json::json!({
        "owner":{"run":1,"connectionGeneration":2,"sessionGeneration":3,"ownerRevision":0,"sceneRevision":4,"hudGeneration":5,"playerObjectId":6},
        "revision":1,"incarnation":7,"draftEpoch":8,"draftGeneration":"18446744073709551615","presentationRevision":9,"layoutRevision":10,
        "open":true,"inputEnabled":true,"kind":"parcel","presentation":{"logicalWidth":600,"logicalHeight":320,"stageCssScale":1,"touch":true},
        "raw":{"to":" Receiver ","subject":"subject remains raw","body":"😀e\u{301}\r\nאבג","goldText":"0007","items":["UID7 display"],"attachmentUniqueIds":["7"],"stamped":true,"attachmentUniqueIdsPresent":true,"stampedPresent":true},
        "gold":7,"parcel":{"stamped":true,"stampAvailable":true,"quoteReady":false,"postage":null,"quoteError":null,"cells":[{"uniqueId":7,"image":116,"countLabel":"2"}]},
        "notice":null,"recipientPrompt":null,"feedback":null,"goldPrompt":null})}
    #[test]
    fn common_host_preserves_every_full_raw_field_and_u64_decimal_generation() {
        let v=compose_value();let s=ComposeUiSnapshot::parse(&v.to_string()).unwrap();
        assert_eq!(serde_json::to_value(&s.raw).unwrap(),v["raw"]);assert_eq!(s.draft_generation,u64::MAX.to_string());
        assert_eq!(s.raw.gold_text,"0007");assert_eq!(s.raw.to," Receiver ");
        let mut unsupported=v.clone();unsupported["presentation"]["logicalWidth"]=599.into();assert!(ComposeUiSnapshot::parse(&unsupported.to_string()).is_err());
        unsupported["presentation"]["logicalWidth"]=390.into();unsupported["presentation"]["logicalHeight"]=844.into();assert!(ComposeUiSnapshot::parse(&unsupported.to_string()).is_err());
    }
    #[test]
    fn common_host_exact_nullable_duplicate_identity_and_presence_fences() {
        for key in ["notice","recipientPrompt","feedback","goldPrompt"] {let mut v=compose_value();v.as_object_mut().unwrap().remove(key);assert!(ComposeUiSnapshot::parse(&v.to_string()).is_err(),"missing nullable {key}");}
        for key in ["to","subject","body","goldText","items","attachmentUniqueIds","stamped","attachmentUniqueIdsPresent","stampedPresent"] {let mut v=compose_value();v["raw"].as_object_mut().unwrap().remove(key);assert!(ComposeUiSnapshot::parse(&v.to_string()).is_err(),"missing raw {key}");}
        for mutate in 0..9 {let mut v=compose_value();match mutate {
            0=>v["owner"]["sceneRevision"]=(SAFE+1).into(),1=>v["draftGeneration"]="01".into(),2=>v["draftGeneration"]="18446744073709551616".into(),
            3=>v["raw"]["attachmentUniqueIds"]=serde_json::json!(["07"]),4=>v["raw"]["attachmentUniqueIdsPresent"]=false.into(),
            5=>v["raw"]["stampedPresent"]=false.into(),6=>v["parcel"]["cells"][0]["uniqueId"]=8.into(),
            7=>v["recipientPrompt"]=serde_json::json!({"draft":"wrong nested prompt"}),_=>v["unknown"]=true.into()}
            assert!(ComposeUiSnapshot::parse(&v.to_string()).is_err(),"mutation {mutate}");}
        let json=compose_value().to_string();let duplicate=json.replacen("\"revision\":1","\"revision\":1,\"revision\":2",1);
        assert!(ComposeUiSnapshot::parse(&duplicate).is_err());
    }
    #[test]
    fn common_host_gold_null_is_unverified_and_existing_crystal_parser_is_the_only_policy() {
        let mut v=compose_value();v["goldPrompt"]=serde_json::json!({"draft":"007","maxAmount":100,"amount":null});
        let s=ComposeUiSnapshot::parse(&v.to_string()).unwrap();let gold=s.gold_prompt.unwrap();
        assert_eq!(mir2_client_bevy::crystal_ui::mail_compose_shared::gold_prompt_amount(&gold),Some(7));
        v["goldPrompt"]["amount"]=8.into();assert!(ComposeUiSnapshot::parse(&v.to_string()).is_err());
        v["goldPrompt"]["amount"]=7.into();assert!(ComposeUiSnapshot::parse(&v.to_string()).is_ok());
        v["recipientPrompt"]="Receiver".into();assert!(ComposeUiSnapshot::parse(&v.to_string()).is_err(),"one active prompt only");
        let normalized=mir2_client_bevy::crystal_ui::mail_compose_shared::replace_gold_prompt(100,"7x9😀9");
        let mut same=mir2_client_bevy::crystal_ui::amount_input::CrystalAmountInput::new(100);same.draft.clear();same.select_all=false;same.push_text("7x9😀9");
        assert_eq!(normalized.draft,same.draft);assert_eq!(normalized.amount,same.amount());
    }
    #[test]
    fn common_host_mailbox_raw_change_requires_authoritative_generation_and_withdraw_keeps_highwater() {
        let mut v=compose_value();v["draftGeneration"]="9".into();let mut box_=ComposeUiMailbox::default();
        assert!(box_.accept(ComposeUiSnapshot::parse(&v.to_string()).unwrap()));assert!(box_.pending());box_.withdraw();assert!(!box_.pending());
        assert!(!box_.accept(ComposeUiSnapshot::parse(&v.to_string()).unwrap()));v["revision"]=2.into();v["raw"]["subject"]="different".into();
        assert!(!box_.accept(ComposeUiSnapshot::parse(&v.to_string()).unwrap()));v["draftGeneration"]="10".into();assert!(box_.accept(ComposeUiSnapshot::parse(&v.to_string()).unwrap()));
        v["revision"]=3.into();v["draftGeneration"]="8".into();assert!(!box_.accept(ComposeUiSnapshot::parse(&v.to_string()).unwrap()));
    }
    #[test]
    fn common_capability_is_independent_and_disabled_on_non_wasm_without_widening_m13() {
        let old:serde_json::Value=serde_json::from_str(&crate::get_mir2_mail_ui_capabilities()).unwrap();
        let new:serde_json::Value=serde_json::from_str(&crate::get_mir2_mail_compose_ui_capabilities()).unwrap();
        let keys=|v:&serde_json::Value|v.as_object().unwrap().keys().cloned().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(keys(&old),["schemaVersion","mailPageAbiVersion","mailIntentAbiVersion","compiled","startup"].into_iter().map(str::to_owned).collect());
        assert_eq!(keys(&new),["schemaVersion","mailComposeUiAbiVersion","textAdapterAbiVersion","compiled","startup"].into_iter().map(str::to_owned).collect());
        #[cfg(not(target_arch="wasm32"))] {assert_eq!(new["compiled"],false);assert_eq!(new["startup"],false);assert_eq!(new["mailComposeUiAbiVersion"],0);}
    }
}
use mir2_client_bevy::{portable_mail_ui::{MailUiContext,MailUiState,MailPointerEdge,MailIntentQueue},crystal_ui::{mail_page_shared::MailPaintAction,spec::CrystalRect}};
fn value()->serde_json::Value {serde_json::json!({"run":1,"connectionGeneration":2,"sessionGeneration":3,"ownerRevision":0,"sceneRevision":4,"hudGeneration":5,"playerObjectId":6,"revision":1,"modelRevision":1,"presentationRevision":1,"open":true,"inputEnabled":true,"presentation":{"logicalWidth":1024,"logicalHeight":768,"stageCssScale":1,"touch":false},"mail":[{"mailId":71,"senderName":"Sender","message":"本文\\r\\nSecond","subject":"","opened":false,"locked":false,"canReply":true,"collected":false,"gold":100,"items":[{"uniqueId":77,"itemIndex":1,"name":null,"key":null,"count":2,"currentDura":3,"maxDura":4,"soulBoundId":0,"gemCount":1,"identified":true,"cursed":false}],"itemCount":1,"dateSentBinaryDatetime":"638962902000000001","metadataKnown":true}]})}
fn setup()->(MailUiContext,MailUiState){let snapshot=MailSnapshot::parse(&value().to_string()).unwrap();let c=MailUiContext{identity:snapshot.identity,revision:1,model_revision:1,presentation_revision:1,active:true,ready:true,input_enabled:true,presentation:None};let mut s=MailUiState::default();s.ingest(&c,snapshot.model());(c,s)}
fn edge(c:&MailUiContext,s:&MailUiState,sequence:u64,phase:&str,pointer:u64)->MailPointerEdge {MailPointerEdge{identity:c.identity,sequence,model_revision:c.model_revision,presentation_revision:c.presentation_revision,render_revision:s.render_revision,pointer_id:pointer,phase:phase.into(),x:5.,y:5.,button:0}}
fn click(c:&MailUiContext,s:&mut MailUiState,out:&mut MailIntentQueue,action:MailPaintAction,sequence:u64){s.controls=vec![(action,CrystalRect::new(0.,0.,20.,20.))];s.process(c,edge(c,s,sequence,"down",1),out);s.process(c,edge(c,s,sequence+1,"up",1),out);}
#[test]
fn mail_host_strict_identity_dates_and_catalog_attachments(){let snapshot=MailSnapshot::parse(&value().to_string()).unwrap();let model=snapshot.model();assert_eq!(model.mails[0].date_sent_binary_datetime,638962902000000001);assert!(model.mails[0].metadata_known);assert_eq!(model.mails[0].items[0].unique_id,Some(77));assert!(model.mails[0].items[0].image.is_some());assert!(!model.mails[0].read);
 for bad in ["9223372036854775808","-9223372036854775809"]{let mut v=value();v["mail"][0]["dateSentBinaryDatetime"]=bad.into();assert!(MailSnapshot::parse(&v.to_string()).is_err());}
 let mut v=value();v["mail"][0]["dateSentBinaryDatetime"]=serde_json::Value::Null;v["mail"][0]["metadataKnown"]=false.into();assert!(!MailSnapshot::parse(&v.to_string()).unwrap().model().mails[0].metadata_known);
 for field in ["mailId","items"]{let mut v=value();if field=="mailId"{v["mail"][0][field]=(SAFE+1).into();}else{v["mail"][0][field][0]["uniqueId"]=(SAFE+1).into();}assert!(MailSnapshot::parse(&v.to_string()).is_err());}
 let mut v=value();v["mail"][0]["items"][0]["image"]=999.into();assert!(MailSnapshot::parse(&v.to_string()).is_err());
 let mut v=value();let duplicate=v["mail"][0].clone();v["mail"].as_array_mut().unwrap().push(duplicate);assert!(MailSnapshot::parse(&v.to_string()).is_err());
}
#[test]
fn mail_mailbox_withdraw_preserves_highwater_and_rejects_old_run(){let mut m=MailMailbox::default();assert!(m.accept(MailSnapshot::parse(&value().to_string()).unwrap()));m.withdraw();assert!(m.take().is_none());assert!(!m.accept(MailSnapshot::parse(&value().to_string()).unwrap()));let mut v=value();v["revision"]=2.into();assert!(m.accept(MailSnapshot::parse(&v.to_string()).unwrap()));v["run"]=2.into();v["revision"]=1.into();assert!(m.accept(MailSnapshot::parse(&v.to_string()).unwrap()));v["run"]=1.into();v["revision"]=99.into();assert!(!m.accept(MailSnapshot::parse(&v.to_string()).unwrap()));}
#[test]
fn mail_controller_read_receipt_is_not_optimistic_and_reader_identity_rebinds_exactly(){let(c,mut s)=setup();let mut out=MailIntentQueue::default();click(&c,&mut s,&mut out,MailPaintAction::Select(71),1);assert!(out.0.is_empty());click(&c,&mut s,&mut out,MailPaintAction::Select(71),3);assert_eq!(out.0.len(),1);assert_eq!(out.0[0].action,"read");assert!(!s.model.mails[0].read);assert!(!s.reader.as_ref().unwrap().read);let mut model=s.model.clone();model.mails[0].read=true;s.ingest(&c,model);assert!(s.reader.as_ref().unwrap().read);let mut model=s.model.clone();model.mails[0].sender="Other".into();s.ingest(&c,model);assert!(s.reader.is_none());assert!(s.model.selected_id.is_none());}
#[test]
fn mail_controller_cancel_second_pointer_and_close_do_not_replay(){let(c,mut s)=setup();let mut out=MailIntentQueue::default();s.controls=vec![(MailPaintAction::Read(71),CrystalRect::new(0.,0.,20.,20.))];s.process(&c,edge(&c,&s,1,"down",1),&mut out);s.process(&c,edge(&c,&s,2,"down",2),&mut out);s.process(&c,edge(&c,&s,3,"up",1),&mut out);assert!(out.0.is_empty());click(&c,&mut s,&mut out,MailPaintAction::Read(71),4);assert_eq!(out.0.len(),1);s.close();s.ingest(&c,MailSnapshot::parse(&value().to_string()).unwrap().model());out.0.clear();click(&c,&mut s,&mut out,MailPaintAction::Read(71),4);assert!(out.0.is_empty());click(&c,&mut s,&mut out,MailPaintAction::Read(71),6);assert_eq!(out.0.len(),1);}
#[test]
fn mail_controller_hidden_unready_and_changed_owner_cannot_consume_old_hold(){let(mut c,mut s)=setup();let mut out=MailIntentQueue::default();s.controls=vec![(MailPaintAction::Read(71),CrystalRect::new(0.,0.,20.,20.))];s.process(&c,edge(&c,&s,1,"down",1),&mut out);c.input_enabled=false;s.process(&c,edge(&c,&s,2,"up",1),&mut out);assert!(out.0.is_empty());c.input_enabled=true;c.ready=false;click(&c,&mut s,&mut out,MailPaintAction::Read(71),3);assert!(out.0.is_empty());c.ready=true;s.controls=vec![(MailPaintAction::Read(71),CrystalRect::new(0.,0.,20.,20.))];s.process(&c,edge(&c,&s,5,"down",1),&mut out);c.identity.scene_revision+=1;s.ingest(&c,MailSnapshot::parse(&value().to_string()).unwrap().model());s.process(&c,edge(&c,&s,6,"up",1),&mut out);assert!(out.0.is_empty());}
#[test]
fn mail_controller_authoritative_flags_gate_claim_delete_and_letter_lock(){let(c,mut s)=setup();let mut out=MailIntentQueue::default();let mut model=s.model.clone();model.mails[0].locked=true;s.ingest(&c,model);s.reader=Some(s.model.mails[0].clone());assert!(!s.allowed(MailPaintAction::ReaderClaim));assert!(!s.allowed(MailPaintAction::Delete(71)));let mut model=s.model.clone();model.mails[0].locked=false;model.mails[0].claimed=true;s.ingest(&c,model);s.reader=Some(s.model.mails[0].clone());assert!(!s.allowed(MailPaintAction::ReaderClaim));let mut model=s.model.clone();model.mails[0].gold=0;model.mails[0].items.clear();s.ingest(&c,model);s.reader=Some(s.model.mails[0].clone());click(&c,&mut s,&mut out,MailPaintAction::ReaderLock,1);assert_eq!(out.0[0].lock,Some(true));assert!(!s.model.mails[0].locked);}
#[test]
#[ignore="Root supplies actual executed Mail JS production setter/router/legacy captures"]
fn m13_actual_js_mail_setter_and_pointer_capture(){let read=|name|std::fs::read_to_string(std::env::var(name).expect("explicit Root fixture input")).unwrap();let captured=read("MIR2_M13_MAIL_SNAPSHOT_FIXTURE");let snapshot=MailSnapshot::parse(&captured).unwrap();assert!(snapshot.open&&snapshot.input_enabled);assert_eq!(snapshot.mail[0].date_sent_binary_datetime.as_deref(),Some("638962902000000001"));assert_eq!(snapshot.model().mails[0].items[0].unique_id,Some(77));let e:MailPointerEdge=serde_json::from_str(&read("MIR2_M13_MAIL_POINTER_FIXTURE")).unwrap();assert_eq!(e.identity,snapshot.identity);assert_eq!(e.model_revision,snapshot.model_revision);assert_eq!(e.presentation_revision,snapshot.presentation_revision);assert_eq!(e.phase,"down");let bundle:serde_json::Value=serde_json::from_str(&read("MIR2_M13_MAIL_LEGACY_FIXTURE")).unwrap();let mut legacy=bundle["legacy"].clone();assert_eq!(legacy[0]["mailId"],72);let mut v=value();v["mail"]=legacy.take();let model=MailSnapshot::parse(&v.to_string()).unwrap().model();assert_eq!(model.mails[0].items[0].name.as_deref(),Some("Wooden Sword"));assert!(!model.mails[0].metadata_known);assert_eq!(model.mails[1].id,73);assert_eq!(model.mails[1].items[0].unique_id,Some(77));assert!(model.mails[1].has_attachment());
 // Node's controlled port is only accepted if the SAME production Rust facade reproduces every raw output.
 for capture in bundle["resolverCaptures"].as_array().unwrap(){let actual:serde_json::Value=serde_json::from_str(&resolve_mail_rows(&capture["input"].to_string()).unwrap()).unwrap();assert_eq!(actual,capture["output"],"actual catalogue facade vs Node port");}
 assert_catalog_defaults();
 for case in bundle["resolutionCases"].as_array().unwrap(){
  let resolved:serde_json::Value=serde_json::from_str(&resolve_mail_rows(&case["current"].to_string()).unwrap()).unwrap();assert_eq!(resolved,case["resolved"]);
  let mut before=serde_json::from_str::<serde_json::Value>(&captured).unwrap();before["mail"]=case["previous"].clone();
  let old=MailSnapshot::parse(&before.to_string()).unwrap();let c=MailUiContext{identity:old.identity,revision:old.revision,model_revision:old.model_revision,presentation_revision:old.presentation_revision,active:true,ready:true,input_enabled:true,presentation:None};
  let mut state=MailUiState::default();state.ingest(&c,old.model());state.model.selected_id=Some(71);state.reader=Some(state.model.mails[0].clone());
  before["mail"]=case["merged"].clone();let next=MailSnapshot::parse(&before.to_string()).unwrap().model();
  let same=case["name"].as_str()==Some("cosmetic-same-template");assert_eq!(next.mails[0].metadata_known,same);
  match case["name"].as_str().unwrap(){"default-spirit"=>assert!(next.mails[0].items[0].identified),"default-mystery"=>assert!(!next.mails[0].items[0].identified),"template-change"=>{assert_eq!(next.mails[0].items[0].item_index,Some(120));assert!(!next.mails[0].items[0].identified);},"explicit-spirit"=>assert!(!next.mails[0].items[0].identified),"explicit-mystery"=>assert!(next.mails[0].items[0].identified),_=>{}}
  state.ingest(&c,next);assert_eq!(state.reader.is_some(),same,"current raw default/template must govern {}",case["name"]);assert_eq!(state.model.selected_id==Some(71),same);
 }
 let c=MailUiContext{identity:snapshot.identity,revision:snapshot.revision,model_revision:snapshot.model_revision,presentation_revision:snapshot.presentation_revision,active:true,ready:true,input_enabled:true,presentation:None};
 let mut state=MailUiState::default();state.ingest(&c,snapshot.model());let mut out=MailIntentQueue::default();click(&c,&mut state,&mut out,MailPaintAction::Select(71),1);click(&c,&mut state,&mut out,MailPaintAction::Select(71),3);assert!(state.reader.is_some());
 let mut refreshed:serde_json::Value=serde_json::from_str(&captured).unwrap();refreshed["mail"]=serde_json::json!([v["mail"][2].clone()]);let next=MailSnapshot::parse(&refreshed.to_string()).unwrap().model();assert_eq!(next.mails[0].items[0].soul_bound_id,-1);assert!(next.mails[0].items[0].identified);assert!(next.mails[0].metadata_known&&next.mails[0].can_reply);
 state.ingest(&c,next);assert_eq!(state.model.selected_id,Some(71));assert!(state.reader.as_ref().unwrap().read);assert_eq!(state.reader.as_ref().unwrap().date_sent_binary_datetime,638962902000000001);
 for (field,changed) in [("uniqueId",serde_json::json!(78)),("count",serde_json::json!(3)),("currentDura",serde_json::json!(4)),("identified",serde_json::json!(false)),("cursed",serde_json::json!(true))]{
  let mut replaced=refreshed.clone();replaced["mail"][0]["items"][0][field]=changed;let mut fresh=MailUiState::default();fresh.ingest(&c,MailSnapshot::parse(&refreshed.to_string()).unwrap().model());fresh.model.selected_id=Some(71);fresh.reader=Some(fresh.model.mails[0].clone());fresh.ingest(&c,MailSnapshot::parse(&replaced.to_string()).unwrap().model());assert!(fresh.reader.is_none(),"replacement {field}");assert!(fresh.model.selected_id.is_none());
 }
}

fn assert_catalog_defaults(){
 // Independently fixed catalogue expectations, exercised through the same public Rust facade.
 for (index,name,expected) in [(1,"SpiritBlade",true),(120,"MysteryHelmet",false)]{
  let mut current=value()["mail"].clone();current[0]["items"][0]["itemIndex"]=index.into();current[0]["items"][0]["name"]=name.into();
  current[0]["items"][0]["key"]=format!("crystal-item-{index}").into();current[0]["items"][0]["identified"]=serde_json::Value::Null;
  let resolved:serde_json::Value=serde_json::from_str(&resolve_mail_rows(&current.to_string()).expect("actual catalogue facade")).unwrap();
  assert_eq!(resolved[0]["items"][0]["itemIndex"],serde_json::json!(index));assert_eq!(resolved[0]["items"][0]["name"],serde_json::json!(name));
  assert_eq!(resolved[0]["items"][0]["identified"],serde_json::json!(expected),"independent current {name} default");
 }
}
#[test]
fn mail_host_nullable_identification_uses_catalog_and_same_uid_labels_do_not_replace_reader(){
 assert_catalog_defaults();
 for (index,old_flag,expected) in [(1,false,true),(120,true,false)]{
  let mut previous=value();previous["mail"][0]["items"][0]["itemIndex"]=index.into();previous["mail"][0]["items"][0]["identified"]=old_flag.into();
  let before=MailSnapshot::parse(&previous.to_string()).unwrap();let c=MailUiContext{identity:before.identity,revision:1,model_revision:1,presentation_revision:1,active:true,ready:true,input_enabled:true,presentation:None};let mut state=MailUiState::default();state.ingest(&c,before.model());state.model.selected_id=Some(71);state.reader=Some(state.model.mails[0].clone());
  let mut current=previous.clone();current["mail"][0]["items"][0]["itemIndex"]=serde_json::Value::Null;current["mail"][0]["items"][0]["key"]=format!("crystal-item-{index}").into();current["mail"][0]["items"][0]["identified"]=serde_json::Value::Null;
  let next=MailSnapshot::parse(&current.to_string()).unwrap().model();assert_eq!(next.mails[0].items[0].identified,expected);assert_eq!(next.mails[0].items[0].item_index,Some(index));state.ingest(&c,next);assert!(state.reader.is_none());assert!(state.model.selected_id.is_none());
  current["mail"][0]["items"][0]["identified"]=old_flag.into();assert_eq!(MailSnapshot::parse(&current.to_string()).unwrap().model().mails[0].items[0].identified,old_flag,"explicit override");
 }
 let (c,mut state)=setup();state.model.selected_id=Some(71);state.reader=Some(state.model.mails[0].clone());let mut enriched=value();enriched["mail"][0]["subject"]="Label enrichment".into();enriched["mail"][0]["items"][0]["name"]="SpiritBlade translated label".into();enriched["mail"][0]["items"][0]["key"]="crystal-item-1".into();enriched["mail"][0]["items"][0]["identified"]=serde_json::Value::Null;
 state.ingest(&c,MailSnapshot::parse(&enriched.to_string()).unwrap().model());assert!(state.reader.is_some());assert_eq!(state.model.selected_id,Some(71));assert!(state.reader.as_ref().unwrap().metadata_known);
 enriched["mail"][0]["items"][0]["itemIndex"]=serde_json::Value::Null;enriched["mail"][0]["items"][0]["key"]="crystal-item-120".into();state.ingest(&c,MailSnapshot::parse(&enriched.to_string()).unwrap().model());assert_eq!(state.model.mails[0].items[0].item_index,Some(120));assert!(state.reader.is_none());
 for (index,key) in [(Some(1),"crystal-item-120"),(None,"crystal-item-2147483647"),(None,"mystery-alias"),(None,"crystal-item-01")]{let mut invalid=value();invalid["mail"][0]["items"][0]["itemIndex"]=serde_json::json!(index);invalid["mail"][0]["items"][0]["key"]=key.into();assert!(MailSnapshot::parse(&invalid.to_string()).is_err());assert!(resolve_mail_rows(&invalid["mail"].to_string()).is_err());}
}


// Source08 differential oracle: this test module is cfg(test); production has no typed Value roots.
fn legacy_mail_snapshot_parse(json:&str)->Result<MailSnapshot,&'static str>{if json.len()>2*1024*1024{return Err("Mail snapshot bound");}
        let raw:serde_json::Value=serde_json::from_str(json).map_err(|_|"Mail schema")?;
        if let Some(rows)=raw.get("mail").and_then(serde_json::Value::as_array){for row in rows{if let Some(items)=row.get("items").and_then(serde_json::Value::as_array){for item in items{
            let keys=["uniqueId","itemIndex","name","key","count","currentDura","maxDura","soulBoundId","gemCount","identified","cursed"];
            if !item.as_object().is_some_and(|m|m.len()==keys.len()&&keys.iter().all(|k|m.contains_key(*k))){return Err("Mail attachment schema");}
        }}}}
        let mut s:MailSnapshot=serde_json::from_value(raw).map_err(|_|"Mail schema")?;s.validate()?;resolve_rows(&mut s.mail)?;Ok(s)}

fn legacy_resolve_mail_rows(json:&str)->Result<String,&'static str>{
    if json.len()>2*1024*1024{return Err("Mail rows bound");}
    let raw:serde_json::Value=serde_json::from_str(json).map_err(|_|"Mail rows schema")?;
    let source=raw.as_array().ok_or("Mail rows schema")?;
    for row in source {for item in row.get("items").and_then(serde_json::Value::as_array).ok_or("Mail rows schema")?{
        let keys=["uniqueId","itemIndex","name","key","count","currentDura","maxDura","soulBoundId","gemCount","identified","cursed"];
        if !item.as_object().is_some_and(|m|m.len()==keys.len()&&keys.iter().all(|k|m.contains_key(*k))){return Err("Mail attachment schema");}
    }}
    let mut rows:Vec<MailRow>=serde_json::from_value(raw).map_err(|_|"Mail rows schema")?;
    validate_rows(&rows)?;resolve_rows(&mut rows)?;
    serde_json::to_string(&rows).map_err(|_|"Mail rows serialize")
}

fn assert_mail_snapshot_fields(actual: &MailSnapshot, expected: &MailSnapshot, label: &str) {
    assert_eq!(actual.identity, expected.identity, "identity: {label}");
    assert_eq!((actual.revision, actual.model_revision, actual.presentation_revision, actual.open, actual.input_enabled),
        (expected.revision, expected.model_revision, expected.presentation_revision, expected.open, expected.input_enabled), "snapshot fields: {label}");
    let presentation_bits = |p: Option<MailPresentation>| p.map(|p| (p.logical_width.to_bits(), p.logical_height.to_bits(), p.stage_css_scale.to_bits(), p.touch));
    assert_eq!(presentation_bits(actual.presentation), presentation_bits(expected.presentation), "presentation bits: {label}");
    assert_eq!(serde_json::to_string(&actual.mail).unwrap(), serde_json::to_string(&expected.mail).unwrap(), "every serialized row/attachment field: {label}");
    let images = |s: &MailSnapshot| s.mail.iter().flat_map(|r| r.items.iter().map(|i| i.resolved_image)).collect::<Vec<_>>();
    assert_eq!(images(actual), images(expected), "skipped resolved image: {label}");
    assert_eq!(actual.model(), expected.model(), "every model field: {label}");
}
fn assert_mail_wire_mapping(raw: &serde_json::Value, label: &str) {
    match (mail_wire::snapshot(raw), serde_json::from_value::<MailSnapshot>(raw.clone())) {
        (Some(actual), Ok(expected)) => assert_mail_snapshot_fields(&actual, &expected, label),
        (None, Err(_)) => (),
        (actual, expected) => panic!("typed snapshot acceptance mismatch {label}: {actual:?} / {expected:?}"),
    }
    if let Some(rows) = raw.get("mail") {
        match (mail_wire::rows(rows), serde_json::from_value::<Vec<MailRow>>(rows.clone())) {
            (Some(actual), Ok(expected)) => {
                assert_eq!(serde_json::to_string(&actual).unwrap(), serde_json::to_string(&expected).unwrap(), "typed rows: {label}");
                for (a, e) in actual.iter().zip(&expected) { for (a, e) in a.items.iter().zip(&e.items) { assert_eq!(a.resolved_image, e.resolved_image, "raw image: {label}"); } }
            },
            (None, Err(_)) => (),
            (actual, expected) => panic!("typed rows acceptance mismatch {label}: {actual:?} / {expected:?}"),
        }
    }
}
fn assert_mail_wire_json(json: &str, label: &str) {
    match (MailSnapshot::parse(json), legacy_mail_snapshot_parse(json)) {
        (Ok(actual), Ok(expected)) => assert_mail_snapshot_fields(&actual, &expected, label),
        (Err(actual), Err(expected)) => assert_eq!(actual, expected, "public snapshot error category: {label}"),
        (actual, expected) => panic!("public snapshot acceptance mismatch {label}: {actual:?} / {expected:?}"),
    }
}
fn assert_mail_wire_case(raw: &serde_json::Value, label: &str) {
    assert_mail_wire_mapping(raw, label);
    assert_mail_wire_json(&raw.to_string(), label);
    if let Some(rows) = raw.get("mail") {
        let json = rows.to_string();
        assert_eq!(resolve_mail_rows(&json), legacy_resolve_mail_rows(&json), "exact facade output or error category: {label}");
    }
}
fn mail_wire_set(raw: &mut serde_json::Value, pointer: &str, changed: serde_json::Value) {
    *raw.pointer_mut(pointer).expect("existing corpus field") = changed;
}
fn mail_wire_sequence(object: &serde_json::Value, fields: &[&str]) -> serde_json::Value {
    serde_json::Value::Array(fields.iter().map(|field| object[*field].clone()).collect())
}
const MAIL_WIRE_ROW_FIELDS: &[&str] = &["mailId", "senderName", "message", "subject", "opened", "locked", "canReply", "collected", "gold", "items", "itemCount", "dateSentBinaryDatetime", "metadataKnown"];
const MAIL_WIRE_ATTACHMENT_FIELDS: &[&str] = &["uniqueId", "itemIndex", "name", "key", "count", "currentDura", "maxDura", "soulBoundId", "gemCount", "identified", "cursed"];

#[test]
fn mail_wire_oracle_preserves_all_fields_models_and_exact_resolved_output() {
    let mut raw = value();
    raw["ownerRevision"] = SAFE.into(); raw["playerObjectId"] = u32::MAX.into();
    raw["revision"] = 12.into(); raw["modelRevision"] = 13.into(); raw["presentationRevision"] = 14.into();
    raw["inputEnabled"] = false.into(); raw["presentation"]["touch"] = true.into(); raw["presentation"]["stageCssScale"] = 2.75.into();
    raw["mail"][0]["senderName"] = "寄件人 😀\nSender".into(); raw["mail"][0]["message"] = "内容\r\n\t第二行\u{0000}".into(); raw["mail"][0]["subject"] = "主题".into();
    raw["mail"][0]["opened"] = true.into(); raw["mail"][0]["locked"] = true.into(); raw["mail"][0]["canReply"] = false.into(); raw["mail"][0]["collected"] = true.into();
    raw["mail"][0]["gold"] = u32::MAX.into(); raw["mail"][0]["dateSentBinaryDatetime"] = i64::MIN.to_string().into();
    let mut items = Vec::new();
    for (index, identified) in [(1, None), (120, None), (1, Some(false)), (120, Some(true))] {
        let mut item = value()["mail"][0]["items"][0].clone();
        item["uniqueId"] = (SAFE - items.len() as u64).into(); item["itemIndex"] = index.into();
        item["name"] = format!("Label {index} 😀").into(); item["key"] = format!("crystal-item-{index}").into();
        item["count"] = u16::MAX.into(); item["currentDura"] = 0.into(); item["maxDura"] = u16::MAX.into();
        item["soulBoundId"] = i32::MIN.into(); item["gemCount"] = u16::MAX.into(); item["identified"] = serde_json::json!(identified); item["cursed"] = true.into();
        items.push(item);
    }
    items.push(serde_json::json!({"uniqueId":null,"itemIndex":null,"name":"Legacy display only","key":"legacy-label","count":0,"currentDura":65535,"maxDura":0,"soulBoundId":2147483647,"gemCount":0,"identified":null,"cursed":false}));
    raw["mail"][0]["items"] = items.into(); raw["mail"][0]["itemCount"] = 5.into();
    let mut second = raw["mail"][0].clone(); second["mailId"] = 72.into(); second["dateSentBinaryDatetime"] = i64::MAX.to_string().into();
    raw["mail"].as_array_mut().unwrap().push(second);
    assert_mail_wire_case(&raw, "field-rich resolved rows");
    let actual = MailSnapshot::parse(&raw.to_string()).unwrap();
    assert_eq!(actual.mail[0].gold, u32::MAX); assert_eq!(actual.mail[0].items[0].soul_bound_id, i32::MIN);
    assert_eq!(actual.mail[0].items.iter().map(|i| i.identified).collect::<Vec<_>>(), vec![Some(true), Some(false), Some(false), Some(true), Some(false)]);
    assert_eq!(actual.model().mails[0].date_sent_binary_datetime, i64::MIN);
    for open in [true, false] { let mut changed = raw.clone(); changed["open"] = open.into(); changed["presentation"] = serde_json::Value::Null; assert_mail_wire_case(&changed, "nullable presentation"); }
}

#[test]
fn mail_wire_oracle_required_optional_unknown_and_scalar_schema_corpus() {
    let levels: &[(&str, &[&str])] = &[
        ("", &["run", "connectionGeneration", "sessionGeneration", "ownerRevision", "sceneRevision", "hudGeneration", "playerObjectId", "revision", "modelRevision", "presentationRevision", "open", "inputEnabled", "presentation", "mail"]),
        ("/presentation", &["logicalWidth", "logicalHeight", "stageCssScale", "touch"]),
        ("/mail/0", MAIL_WIRE_ROW_FIELDS), ("/mail/0/items/0", MAIL_WIRE_ATTACHMENT_FIELDS),
    ];
    let variants = [serde_json::Value::Null, serde_json::json!(false), serde_json::json!(0), serde_json::json!(-1), serde_json::json!(1.0), serde_json::json!("1"), serde_json::json!([]), serde_json::json!({})];
    for (parent, fields) in levels {
        for field in *fields {
            let pointer = format!("{parent}/{field}");
            let mut missing = value(); missing.pointer_mut(parent).unwrap().as_object_mut().unwrap().remove(*field);
            assert_mail_wire_case(&missing, &format!("missing {pointer}"));
            for variant in &variants {
                let mut changed = value(); mail_wire_set(&mut changed, &pointer, variant.clone());
                assert_mail_wire_case(&changed, &format!("type {pointer}={variant}"));
            }
        }
        let mut extra = value(); extra.pointer_mut(parent).unwrap().as_object_mut().unwrap().insert("unexpected".into(), serde_json::Value::Null);
        assert_mail_wire_case(&extra, &format!("unknown {parent}"));
    }
    for field in ["identity", "player_object_id", "version"] { let mut raw = value(); raw[field] = serde_json::json!({"run":1}); assert_mail_wire_case(&raw, field); }
    for field in ["resolvedImage", "resolved_image", "image"] { let mut raw = value(); raw["mail"][0]["items"][0][field] = 1.into(); assert_mail_wire_case(&raw, field); assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail attachment schema"); }
    for raw in [serde_json::Value::Null, serde_json::json!(false), serde_json::json!(1), serde_json::json!("snapshot"), serde_json::json!([])] { assert_mail_wire_case(&raw, "top-level type"); }
}

#[test]
fn mail_wire_oracle_integer_ranges_and_target_usize_error_categories() {
    let unsigned = ["0", "1", "65535", "65536", "4294967295", "4294967296", "9007199254740991", "9007199254740992", "9223372036854775807", "9223372036854775808", "18446744073709551615", "18446744073709551616", "-1", "1.0", "-0.0", "1e0", "\"1\""];
    for pointer in ["/run", "/connectionGeneration", "/sessionGeneration", "/ownerRevision", "/sceneRevision", "/hudGeneration", "/playerObjectId", "/revision", "/modelRevision", "/presentationRevision", "/mail/0/mailId", "/mail/0/gold", "/mail/0/itemCount", "/mail/0/items/0/uniqueId", "/mail/0/items/0/count", "/mail/0/items/0/currentDura", "/mail/0/items/0/maxDura", "/mail/0/items/0/gemCount"] {
        for number in unsigned { let mut raw = value(); mail_wire_set(&mut raw, pointer, serde_json::from_str(number).unwrap()); assert_mail_wire_case(&raw, &format!("unsigned {pointer}={number}")); }
    }
    for pointer in ["/mail/0/items/0/itemIndex", "/mail/0/items/0/soulBoundId"] {
        for number in ["-2147483649", "-2147483648", "-1", "0", "1", "120", "2147483647", "2147483648", "9223372036854775807", "-9223372036854775808", "18446744073709551615", "1.0", "-0.0", "\"1\""] {
            let mut raw = value(); mail_wire_set(&mut raw, pointer, serde_json::from_str(number).unwrap()); assert_mail_wire_case(&raw, &format!("signed {pointer}={number}"));
        }
    }
    for number in [u32::MAX as u64, u32::MAX as u64 + 1, u64::MAX] {
        let mut raw = value(); raw["mail"][0]["itemCount"] = number.into();
        let fits = usize::try_from(number).is_ok();
        assert_eq!(mail_wire::snapshot(&raw).is_some(), fits, "target pointer width {}", usize::BITS);
        assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), if fits { "Mail row bound/identity" } else { "Mail schema" });
        assert_eq!(resolve_mail_rows(&raw["mail"].to_string()).unwrap_err(), if fits { "Mail row bound/identity" } else { "Mail rows schema" });
        assert_mail_wire_case(&raw, "target usize");
    }
}

#[test]
fn mail_wire_oracle_f32_cast_rounding_sign_and_validation_order() {
    for pointer in ["/presentation/logicalWidth", "/presentation/logicalHeight", "/presentation/stageCssScale"] {
        for number in ["0", "-0", "0.0", "-0.0", "1", "1024", "768", "16384", "16384.0001", "0.000000000000000000000000000000000000000000001", "-1e-300", "1e300", "-1e300", "3.4028234663852886e38", "3.4028236e38", "16777217", "9223372586610589697", "-9223371487098961919", "18446743523953737727", "1e309", "\"1024\""] {
            let mut raw = value();
            if let Ok(n) = serde_json::from_str(number) { mail_wire_set(&mut raw, pointer, n); assert_mail_wire_case(&raw, &format!("float {pointer}={number}")); }
            else { let json = raw.to_string().replace("\"logicalWidth\":1024", &format!("\"logicalWidth\":{number}")); assert_mail_wire_json(&json, "JSON float overflow"); }
        }
    }
    let mut raw = value(); raw["presentation"]["logicalWidth"] = serde_json::json!(1e300);
    assert!(mail_wire::snapshot(&raw).unwrap().presentation.unwrap().logical_width.is_infinite());
    assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail owner/presentation bound");
    for scale in [2.7499999, 2.75, 16.0, 16.000001] { let mut raw = value(); raw["presentation"]["touch"] = true.into(); raw["presentation"]["stageCssScale"] = scale.into(); assert_mail_wire_case(&raw, "touch scale rounding boundary"); }
}


#[test]
fn mail_wire_oracle_struct_sequences_lengths_and_attachment_precheck_asymmetry() {
    let mut raw = value();
    let presentation = mail_wire_sequence(&raw["presentation"], &["logicalWidth", "logicalHeight", "stageCssScale", "touch"]);
    for len in 0..=5 {
        let mut changed = raw.clone(); let mut seq = presentation.as_array().unwrap().clone(); seq.resize(len, serde_json::Value::Null); changed["presentation"] = seq.into();
        assert_mail_wire_case(&changed, &format!("presentation sequence length {len}"));
        assert_eq!(MailSnapshot::parse(&changed.to_string()).is_ok(), len == 4);
    }
    raw["presentation"] = presentation;
    let row = mail_wire_sequence(&raw["mail"][0], MAIL_WIRE_ROW_FIELDS);
    for len in 0..=14 {
        let mut changed = raw.clone(); let mut seq = row.as_array().unwrap().clone(); seq.resize(len, serde_json::Value::Null); changed["mail"][0] = seq.into();
        assert_mail_wire_case(&changed, &format!("row sequence length {len}"));
        assert_eq!(MailSnapshot::parse(&changed.to_string()).is_ok(), len == 13);
        assert_eq!(resolve_mail_rows(&changed["mail"].to_string()).unwrap_err(), "Mail rows schema");
    }
    raw["mail"][0] = row;
    let attachment = mail_wire_sequence(&raw["mail"][0][9][0], MAIL_WIRE_ATTACHMENT_FIELDS);
    for len in 0..=12 {
        let mut changed = raw.clone(); let mut seq = attachment.as_array().unwrap().clone(); seq.resize(len, serde_json::Value::Null); changed["mail"][0][9][0] = seq.into();
        assert_mail_wire_case(&changed, &format!("attachment sequence length {len}"));
        assert_eq!(MailSnapshot::parse(&changed.to_string()).is_ok(), len == 11);
    }
    raw["mail"][0][9][0] = attachment.clone();
    assert_mail_wire_case(&raw, "row and attachment sequences accepted in snapshot");
    assert!(MailSnapshot::parse(&raw.to_string()).is_ok());
    let mut ordinary = value(); ordinary["mail"][0]["items"][0] = attachment;
    assert_mail_wire_case(&ordinary, "ordinary object row requires attachment object");
    assert_eq!(MailSnapshot::parse(&ordinary.to_string()).unwrap_err(), "Mail attachment schema");
    assert_eq!(resolve_mail_rows(&ordinary["mail"].to_string()).unwrap_err(), "Mail attachment schema");
    raw["mail"][0][9][0] = serde_json::json!({"count":2,"currentDura":3,"maxDura":4,"soulBoundId":-1,"gemCount":1,"cursed":false});
    assert_mail_wire_case(&raw, "optional attachment fields omitted behind row sequence");
    let item = &MailSnapshot::parse(&raw.to_string()).unwrap().model().mails[0].items[0];
    assert_eq!((item.unique_id, item.item_index, item.name.as_deref(), item.key.as_deref(), item.image, item.identified), (None, None, None, None, None, false));
    let mut unknown = raw.clone(); unknown["mail"][0][9][0]["resolvedImage"] = 1.into(); assert_mail_wire_case(&unknown, "sequence bypass still rejects unknown attachment field");
    for index in 0..13 { let mut bad = raw.clone(); bad["mail"][0][index] = serde_json::json!({}); assert_mail_wire_case(&bad, &format!("row sequence bad slot {index}")); }
    assert_mail_wire_case(&mail_wire_sequence(&value(), &["run", "connectionGeneration", "sessionGeneration", "ownerRevision", "sceneRevision", "hudGeneration", "playerObjectId", "revision", "modelRevision", "presentationRevision", "open", "inputEnabled", "presentation", "mail"]), "flatten snapshot never accepts sequence");
}

#[test]
fn mail_wire_oracle_exact_two_mib_bounds_and_public_error_precedence() {
    let snapshot_json = value().to_string(); let rows_json = value()["mail"].to_string();
    let snapshot_at_limit = format!("{snapshot_json}{}", " ".repeat(2 * 1024 * 1024 - snapshot_json.len()));
    let rows_at_limit = format!("{rows_json}{}", " ".repeat(2 * 1024 * 1024 - rows_json.len()));
    assert_eq!(snapshot_at_limit.len(), 2 * 1024 * 1024); assert_eq!(rows_at_limit.len(), 2 * 1024 * 1024);
    assert_mail_wire_json(&snapshot_at_limit, "exact snapshot byte bound"); assert!(MailSnapshot::parse(&snapshot_at_limit).is_ok());
    assert_eq!(resolve_mail_rows(&rows_at_limit), legacy_resolve_mail_rows(&rows_at_limit)); assert!(resolve_mail_rows(&rows_at_limit).is_ok());
    let snapshot_over = format!("{snapshot_at_limit} "); let rows_over = format!("{rows_at_limit} ");
    assert_mail_wire_json(&snapshot_over, "one byte over snapshot bound"); assert_eq!(MailSnapshot::parse(&snapshot_over).unwrap_err(), "Mail snapshot bound");
    assert_eq!(resolve_mail_rows(&rows_over), legacy_resolve_mail_rows(&rows_over)); assert_eq!(resolve_mail_rows(&rows_over).unwrap_err(), "Mail rows bound");
    for json in ["", "{", "[", "{\"mail\":", "null", "[null]", "{}"] { assert_mail_wire_json(json, "malformed/schema JSON"); assert_eq!(resolve_mail_rows(json), legacy_resolve_mail_rows(json)); }
    let mut raw = value(); raw["run"] = 0.into(); raw["mail"][0]["senderName"] = serde_json::Value::Null; raw["mail"][0]["items"][0].as_object_mut().unwrap().remove("uniqueId");
    assert_mail_wire_case(&raw, "attachment precheck precedes schema and owner validation"); assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail attachment schema");
    raw["mail"][0]["items"][0]["uniqueId"] = 77.into(); assert_mail_wire_case(&raw, "typed schema precedes owner validation"); assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail schema");
    raw["mail"][0]["senderName"] = "Sender".into(); raw["mail"][0]["mailId"] = 0.into(); assert_mail_wire_case(&raw, "owner precedes row validation"); assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail owner/presentation bound");
    raw["run"] = 1.into(); raw["mail"][0]["items"][0]["itemIndex"] = i32::MAX.into(); assert_mail_wire_case(&raw, "row identity precedes catalog resolution"); assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail row bound/identity");
    raw["mail"][0]["mailId"] = 71.into(); assert_mail_wire_case(&raw, "catalog error retained"); assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail unknown current item index");
}

#[test]
fn mail_wire_oracle_semantic_identity_lengths_dates_counts_and_catalog_cases() {
    for count in [0, 1, 256, 257] {
        let mut raw = value(); let row = raw["mail"][0].clone();
        raw["mail"] = (0..count).map(|n| { let mut row = row.clone(); row["mailId"] = (n + 1).into(); row }).collect::<Vec<_>>().into();
        assert_mail_wire_case(&raw, &format!("row count {count}"));
        if count == 257 { assert_eq!(MailSnapshot::parse(&raw.to_string()).unwrap_err(), "Mail owner/presentation bound"); assert_eq!(resolve_mail_rows(&raw["mail"].to_string()).unwrap_err(), "Mail row bound"); }
    }
    for count in [0, 1, 5, 6] {
        let mut raw = value(); let item = raw["mail"][0]["items"][0].clone(); raw["mail"][0]["items"] = vec![item; count].into(); raw["mail"][0]["itemCount"] = count.into();
        assert_mail_wire_case(&raw, &format!("attachment count {count}"));
        assert_eq!(MailSnapshot::parse(&raw.to_string()).is_ok(), count <= 5);
    }
    let mut duplicate = value(); duplicate["mail"].as_array_mut().unwrap().push(value()["mail"][0].clone()); assert_mail_wire_case(&duplicate, "repeated mail id");
    assert_eq!(MailSnapshot::parse(&duplicate.to_string()).unwrap_err(), "Mail row bound/identity");
    for (pointer, max) in [("/mail/0/senderName", 1024), ("/mail/0/message", 262144), ("/mail/0/subject", 16384), ("/mail/0/items/0/name", 2048), ("/mail/0/items/0/key", 2048)] {
        for extra in [0, 1] {
            let mut raw = value(); raw["mail"][0]["items"][0]["uniqueId"] = serde_json::Value::Null; raw["mail"][0]["items"][0]["itemIndex"] = serde_json::Value::Null;
            mail_wire_set(&mut raw, pointer, "x".repeat(max + extra).into()); assert_mail_wire_case(&raw, &format!("UTF8 byte length {pointer} {}", max + extra));
            assert_eq!(MailSnapshot::parse(&raw.to_string()).is_ok(), extra == 0);
        }
    }
    let mut unicode = value(); unicode["mail"][0]["senderName"] = "中".repeat(342).into(); assert_mail_wire_case(&unicode, "UTF8 bytes rather than character count"); assert_eq!(MailSnapshot::parse(&unicode.to_string()).unwrap_err(), "Mail row bound/identity");
    for date in [None, Some("0"), Some("-0"), Some("+1"), Some("0001"), Some("-9223372036854775808"), Some("9223372036854775807"), Some("9223372036854775808"), Some("-9223372036854775809"), Some(" 1"), Some("1 "), Some("1.0"), Some("")] {
        for known in [false, true] { let mut raw = value(); raw["mail"][0]["dateSentBinaryDatetime"] = serde_json::json!(date); raw["mail"][0]["metadataKnown"] = known.into(); assert_mail_wire_case(&raw, &format!("date {date:?} known {known}")); }
    }
    let mut omitted_date = value(); omitted_date["mail"][0].as_object_mut().unwrap().remove("dateSentBinaryDatetime"); omitted_date["mail"][0]["metadataKnown"] = false.into(); assert_mail_wire_case(&omitted_date, "absent optional date"); assert!(MailSnapshot::parse(&omitted_date.to_string()).is_ok());
    for (index, key, uid, name) in [
        (Some(1), None, Some(77), None), (Some(120), Some("crystal-item-120"), Some(77), Some("Current label")),
        (None, Some("crystal-item-1"), Some(77), None), (Some(1), Some("crystal-item-120"), Some(77), None),
        (None, Some("crystal-item-01"), Some(77), None), (None, Some("crystal-item--1"), Some(77), None),
        (None, Some("crystal-item-2147483647"), Some(77), None), (None, Some("unknown"), Some(77), None),
        (None, None, Some(77), None), (None, None, None, Some("Wooden Sword")), (None, Some("legacy"), None, Some("Legacy display only")),
    ] {
        for identified in [None, Some(false), Some(true)] {
            let mut raw = value(); let item = &mut raw["mail"][0]["items"][0]; item["itemIndex"] = serde_json::json!(index); item["key"] = serde_json::json!(key); item["uniqueId"] = serde_json::json!(uid); item["name"] = serde_json::json!(name); item["identified"] = serde_json::json!(identified);
            assert_mail_wire_case(&raw, &format!("catalog {index:?}/{key:?}/{uid:?}/{identified:?}"));
        }
    }
}

#[test]
fn mail_wire_oracle_preserves_value_duplicate_collapse_and_pointer_duplicate_rejection() {
    let snapshot_json = value().to_string();
    for (field, duplicate) in [("run", "2"), ("ownerRevision", "4"), ("open", "false"), ("presentation", "null")] {
        let first = format!("{{\"{field}\":{duplicate},{}", &snapshot_json[1..]);
        let last = format!("{},\"{field}\":{duplicate}}}", &snapshot_json[..snapshot_json.len() - 1]);
        assert_mail_wire_json(&first, &format!("duplicate snapshot first {field}")); assert_mail_wire_json(&last, &format!("duplicate snapshot last {field}"));
    }
    let rows_json = value()["mail"].to_string();
    let duplicate_rows = rows_json.replacen("\"mailId\":71", "\"mailId\":72,\"mailId\":71", 1);
    assert_eq!(resolve_mail_rows(&duplicate_rows), legacy_resolve_mail_rows(&duplicate_rows)); assert!(resolve_mail_rows(&duplicate_rows).is_ok());
    let snapshot = MailSnapshot::parse(&snapshot_json).unwrap();
    let pointer_json = serde_json::json!({"run":snapshot.identity.run,"connectionGeneration":snapshot.identity.connection_generation,"sessionGeneration":snapshot.identity.session_generation,"ownerRevision":snapshot.identity.owner_revision,"sceneRevision":snapshot.identity.scene_revision,"hudGeneration":snapshot.identity.hud_generation,"playerObjectId":snapshot.identity.player_object_id,"sequence":1,"modelRevision":1,"presentationRevision":1,"renderRevision":1,"pointerId":1,"phase":"down","x":5,"y":5,"button":0}).to_string();
    assert!(serde_json::from_str::<MailPointerEdge>(&pointer_json).is_ok());
    for field in ["run", "connectionGeneration", "playerObjectId", "sequence", "x", "button"] {
        let json = format!("{{\"{field}\":1,{}", &pointer_json[1..]); assert!(serde_json::from_str::<MailPointerEdge>(&json).is_err(), "direct pointer duplicate {field}");
    }
}

mod common_compose_candidate_repair02 {
    use super::*;
    use bevy::prelude::*;
    use mir2_client_bevy::{
        crystal_ui::{hud::SharedHudSurface,mail_editor::{MailEditorLayoutSet,MailLetterEditor,MailLetterEditText},
            mail_compose_shared::{ComposeIdentity,ComposeKind,ComposeLayout,ComposePresentation,ComposeFullRaw,ParcelPaint,TextEdge,TextOperation,parse_text_edge}},
        portable_mail_ui::{Mir2PortableMailComposeUiPlugin,PortableComposeSurface,PortableComposeRoot,PortableComposeEdges,PortableComposeIntents},
    };
    #[derive(Resource,Default)]struct PublishedCapture(Vec<u64>);
    fn show_roots(mut roots:Query<&mut Visibility,With<PortableComposeRoot>>) {
        for mut root in &mut roots{*root=Visibility::Inherited;}
    }
    // This consumer is outside client-bevy and only names its public ordering API.
    // A ready observation must use the actual shaped text and computed viewport.
    fn publish_after_capture(editor:Res<MailLetterEditor>,surface:Res<SharedHudSurface>,c:Res<PortableComposeSurface>,
        texts:Query<(&MailLetterEditText,&ComputedNode,&TextFont,&bevy::text::TextLayoutInfo)>,mut out:ResMut<PublishedCapture>) {
        if !editor.layout_ready(){return;}
        for(tag,node,font,info)in &texts {
            if tag.revision!=editor.revision()||tag.layout_revision!=editor.layout_revision(){continue;}
            let body=c.layout.as_ref().unwrap().body;
            assert_eq!(tag.viewport,[body.width-4.,body.height-4.]);
            assert!((node.size().x*node.inverse_scale_factor-tag.viewport[0]).abs()<1.);
            assert_eq!(font.font,bevy::text::FontSource::Handle(surface.font.clone()));
            assert_eq!(font.font_size,FontSize::Px(c.layout.as_ref().unwrap().body_font_px));
            assert!(!info.glyphs.is_empty());out.0.push(tag.layout_revision);
        }
    }
    fn proof(app:&App,sequence:u64)->TextProof {
        app.world().resource::<PortableComposeSurface>().proof(app.world().resource::<MailLetterEditor>(),sequence).unwrap()
    }
    #[test]
    fn common_public_capture_set_orders_actual_cross_crate_publisher_and_queued_paste_wire() {
        use bevy::camera::{ComputedCameraValues,RenderTargetInfo,Viewport};
        let mut app=App::new();
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default(),bevy::input::InputPlugin,
            bevy::image::ImagePlugin::default(),bevy::transform::TransformPlugin,bevy::camera::visibility::VisibilityPlugin,
            bevy::text::TextPlugin,bevy::ui::UiPlugin));
        app.init_asset::<Image>().init_asset::<Font>().init_asset::<bevy::image::TextureAtlasLayout>()
            .init_asset::<bevy::mesh::Mesh>().init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::from_bytes(std::fs::read(
            concat!(env!("CARGO_MANIFEST_DIR"),"/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf")).unwrap()));
        let camera=app.world_mut().spawn((Camera2d,Camera{computed:ComputedCameraValues{target_info:Some(RenderTargetInfo{
            physical_size:UVec2::new(600,320),scale_factor:1.}),..default()},viewport:Some(Viewport{physical_size:UVec2::new(600,320),..default()}),..default()})).id();
        app.insert_resource(SharedHudSurface{camera,window:Entity::PLACEHOLDER,font,active:true,generation:5,revision:1});
        let owner=ComposeIdentity{run:1,connection_generation:2,session_generation:3,owner_revision:0,scene_revision:4,hud_generation:5,player_object_id:6};
        let presentation=ComposePresentation{logical_width:600.,logical_height:320.,stage_css_scale:1.,touch:true};
        let layout=ComposeLayout::for_presentation(presentation,ComposeKind::Letter).unwrap();
        let body="a😀e\u{301}אב中\n".repeat(8);
        let raw=ComposeFullRaw{to:"Receiver".into(),subject:" preserved subject ".into(),body:body.clone(),gold_text:"0007".into(),
            items:vec!["UID7".into()],attachment_unique_ids:vec!["7".into()],stamped:true,attachment_unique_ids_present:true,stamped_present:true};
        let mut editor=MailLetterEditor::default();assert!(editor.mount_external(&body,9,layout.body,12));editor.set_dom_selection(0,0).unwrap();
        app.insert_resource(editor).insert_resource(PortableComposeSurface{owner,revision:1,incarnation:7,draft_epoch:8,draft_generation:"9".into(),
            presentation_revision:10,layout_revision:12,active:true,ready:true,input_enabled:true,kind:Some(ComposeKind::Letter),layout:Some(layout),
            presentation:Some(presentation),raw:Some(raw.clone()),parcel:ParcelPaint::default(),..default()});
        {let mut c=app.world_mut().resource_mut::<PortableComposeSurface>();c.draft.recipient=raw.to.clone();c.draft.message=body.clone();
            c.draft.gold=7;c.draft.attachment_unique_ids=vec![7];}
        app.add_plugins(Mir2PortableMailComposeUiPlugin).init_resource::<PublishedCapture>()
            .add_systems(Update,show_roots.after(mir2_client_bevy::portable_mail_ui::paint_compose))
            .add_systems(PostUpdate,publish_after_capture.after(MailEditorLayoutSet));
        for _ in 0..6{app.update();}
        assert!(app.world().resource::<PublishedCapture>().0.contains(&12));
        let presentation=ComposePresentation{logical_width:844.,logical_height:390.,stage_css_scale:1.,touch:true};
        let layout=ComposeLayout::for_presentation(presentation,ComposeKind::Letter).unwrap();
        assert!(app.world_mut().resource_mut::<MailLetterEditor>().configure_view(layout.body,13));
        assert!(!app.world().resource::<MailLetterEditor>().layout_ready());
        {let mut c=app.world_mut().resource_mut::<PortableComposeSurface>();c.presentation=Some(presentation);c.layout=Some(layout);
            c.layout_revision=13;c.presentation_revision+=1;c.revision+=1;}
        {let mut c=app.world_mut().get_mut::<Camera>(camera).unwrap();c.computed.target_info.as_mut().unwrap().physical_size=UVec2::new(844,390);
            c.viewport.as_mut().unwrap().physical_size=UVec2::new(844,390);}
        for _ in 0..6{app.update();}
        assert!(app.world().resource::<PublishedCapture>().0.contains(&13));
        assert_eq!(app.world().resource::<PortableComposeSurface>().raw.as_ref(),Some(&raw));
        let edge=TextEdge{proof:proof(&app,1),operation:TextOperation::ClipboardRequest{request_id:1,kind:"paste".into()}};
        let json=serde_json::to_string(&edge).unwrap();
        app.world_mut().resource_mut::<PortableComposeEdges>().0.push(parse_text_edge(&json).unwrap());app.update();
        let request=app.world_mut().resource_mut::<PortableComposeIntents>().0.remove(0);
        let wire=serde_json::to_value(&request).unwrap();assert_eq!(request.base_raw,raw);assert!(wire["selectedText"].is_null());
        assert_eq!(wire["clipboardKind"],"paste");assert_eq!(wire["requestId"],1);assert_eq!(request.proof,edge.proof);
        assert_eq!(app.world().resource::<PortableComposeSurface>().draft.message,body);
        let edge=TextEdge{proof:proof(&app,2),operation:TextOperation::ClipboardResult{request_id:1,success:true,text:Some("中".into())}};
        let json=serde_json::to_string(&edge).unwrap();
        app.world_mut().resource_mut::<PortableComposeEdges>().0.push(parse_text_edge(&json).unwrap());app.update();
        let edit=app.world_mut().resource_mut::<PortableComposeIntents>().0.remove(0);
        assert_eq!(edit.base_raw,raw);assert_eq!(edit.body_mutation.as_deref(),Some(format!("中{body}").as_str()));
        let c=app.world().resource::<PortableComposeSurface>();assert!(c.awaiting_draft);assert!(!c.ready);
        assert_eq!(c.draft_generation,"9","only the later cached Core facade may accept and advance the full raw clock");
    }
}

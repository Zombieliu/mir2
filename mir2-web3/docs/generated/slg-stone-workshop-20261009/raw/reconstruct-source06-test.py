from pathlib import Path
import hashlib, json
base=Path(__file__).resolve().parent
repo=base.parent.parent
path='mir2-web3/apps/simulation/src/runtime/stones_tests.rs'
data=(repo/path).read_bytes()
start=data.rfind(b'#[test]',0,data.index(b'fn stone_save_reload_keeps_original_secret_and_physical_reference'))
end=data.index(b'#[test]',start+7)
old='''#[test]
fn stone_save_reload_keeps_original_secret_and_physical_reference() {
    let mut f=Fixture::new("reload",827);let serial=f.seal();let frozen=f.disk().sealed_stones.record(serial).unwrap().clone();
    let mut fresh=login(f.config.clone());fresh.begin_stone_workshops().unwrap();fresh.interact(f.npc);
    assert_eq!(fresh.stone_workshop_view().unwrap().stones[0].serial,serial.to_string());
    let row=&fresh.app.world().resource::<InventoryResource>().inventory_items[0];assert_eq!(row.stone_serial,Some(serial));
    let public=serde_json::to_string(&fresh.stone_workshop_view().unwrap().stones).unwrap();
    assert!(!public.contains("privateOutput"));assert!(!public.contains("roll"));assert!(!public.contains("history"));
    assert_eq!(f.disk().sealed_stones.record(serial).unwrap(),&frozen);
}
'''
expected=next(r['sha256'] for r in json.loads((base/'stone-runtime-authored-source-06.json').read_text())['files'] if r['path']==path)
for newline in ['\n','\r\n']:
    restored=data[:start]+old.replace('\n',newline).encode()+data[end:]
    if hashlib.sha256(restored).hexdigest()==expected:
        (base/'source06-stones-tests-reconstructed.rs').write_bytes(restored)
        print(json.dumps(dict(sha256=expected,length=len(restored),reconstruction=True,productModified=False)))
        break
else: raise RuntimeError('Inverse did not restore approved Source06 bytes')

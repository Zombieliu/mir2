//! Prepared local doorway stories for three classes. Every accepted coordinate
//! comes from a real shared Zone Walk over the packaged Crystal collision.
//! Starting adjacent to each doorway is explicit fixture preparation: these
//! tests are not continuous natural journeys, Boss loot, public WSS, or native
//! visual acceptance. Conditional NPC routes live in classic_late_route_gates.
use std::collections::{BTreeMap, BTreeSet};

use mir2_game_data::crystal_map_respawns_ref;
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    set_crystal_full_world_zone_collision, AccountRecord, CharacterRecord, CharacterSaveRecord,
    InProcessWorldRuntime, SimulationConfig, WorldCommand, WorldRuntime, ZoneCollision,
    ZoneCommand, ZoneKey, ZoneOutbound, ZoneRuntime,
};

const ROUTES: &[(&str, &[&str])] = &[
    ("stone", &["0","2","3","D710","D711","D712","D713","D714","D715","D71601","D71621","D71608","D71605","D71625","D716","D717"]),
    ("stone-return", &["D717","D716","D71625","D71623","D71601","D715","D714","D713","D712","D711","D710","3","2","0"]),
    ("zuma", &["0","2","3","0157","D501","D502","D503","D504","D505","D5061","D5068","D5071","D5072","D5073","D5074","D515"]),
    ("zuma-return", &["D515","D5074","D5073","D5072","D5071","D5068","D5061","D505","D504","D503","D502","D501","0157","3","2","0"]),
    ("redmoon", &["0","1","11","12","D10011","D1002","D10031","D1004","D10052","D10062"]),
    ("redmoon-return", &["D10062","D10052","D1004","D10031","D1002","D10011","12","11","1","0"]),
];

struct SourceCollision {
    collision: ZoneCollision,
    min_x:i32, max_x:i32, min_y:i32, max_y:i32,
    blocked:BTreeSet<(i32,i32)>,
    direct_sources:BTreeSet<(i32,i32)>,
}

impl SourceCollision {
    fn load(map:&str)->Self {
        let collision=ZoneCollision::for_map(map);
        let value=serde_json::to_value(&collision).unwrap();
        let bounds=&value["bounds"];
        assert!(!bounds.is_null(),"{map} must use packaged original collision, never unbounded fallback");
        let number=|key:&str|i32::try_from(bounds[key].as_i64().unwrap()).unwrap();
        let blocked=value["blocked_cells"].as_array().unwrap().iter().map(|cell|(
            i32::try_from(cell[0].as_i64().unwrap()).unwrap(),
            i32::try_from(cell[1].as_i64().unwrap()).unwrap(),
        )).collect();
        let direct_sources=value["transfer_source_cells"].as_array().unwrap().iter().map(|cell|(
            i32::try_from(cell[0].as_i64().unwrap()).unwrap(),
            i32::try_from(cell[1].as_i64().unwrap()).unwrap(),
        )).collect();
        Self { collision,min_x:number("min_x"),max_x:number("max_x"),
            min_y:number("min_y"),max_y:number("max_y"),blocked,direct_sources }
    }

    fn adjacent(&self,source:&Point,map:&str)->(Point,MirDirection) {
        // Choose an actually walkable neighbouring start, never erase walls or
        // mutate terrain to make a manifest doorway work.
        let choices=[(0,-1,MirDirection::Down),(1,-1,MirDirection::DownLeft),
            (1,0,MirDirection::Left),(1,1,MirDirection::UpLeft),(0,1,MirDirection::Up),
            (-1,1,MirDirection::UpRight),(-1,0,MirDirection::Right),(-1,-1,MirDirection::DownRight)];
        choices.into_iter().find_map(|(dx,dy,direction)| {
            let point=Point{x:source.x+dx,y:source.y+dy};
            (point.x>=self.min_x && point.x<=self.max_x && point.y>=self.min_y && point.y<=self.max_y
                && !self.blocked.contains(&(point.x,point.y))).then_some((point,direction))
        }).unwrap_or_else(||panic!("{map} doorway {source:?} has no original walkable adjacent tile"))
    }
}

fn prepared_actor(config:&SimulationConfig,class:MirClass,map:&str,position:Point)->InProcessWorldRuntime {
    let character=CharacterRecord{index:0,name:"PreparedDoorwayStory".into(),level:50,
        class,gender:MirGender::Male};
    let mut save=CharacterSaveRecord::new(character.clone());
    save.map_file_name=map.into();
    save.map_title=map.into();
    save.position=position;
    save.direction=MirDirection::Down;
    let mut account=AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0,save);
    config.account_store.lock().unwrap().accounts.insert("prepared-classic-doorway".into(),account);
    let mut runtime=InProcessWorldRuntime::new(config.clone());
    let login=runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login{
        account_id:"prepared-classic-doorway".into(),password:"demo".into(),
    })).unwrap();
    assert!(login.iter().any(|p|matches!(p,ServerPacket::LoginSuccess{..})),"prepared authenticated Login");
    let start=runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame{character_index:0})).unwrap();
    assert!(start.iter().any(|p|matches!(p,ServerPacket::MapInformation{info} if info.file_name==map)),"prepared source map StartGame");
    runtime
}

fn run_doorway(config:&SimulationConfig,class:MirClass,from:&str,to:&str,collision:&SourceCollision) {
    let source=crystal_map_respawns_ref(from).unwrap();
    let target=crystal_map_respawns_ref(to).unwrap();
    let movement=source.movements.iter().find(|move_|move_.map_index==target.map_index
        && !move_.need_move && !move_.need_hole && move_.conquest_index==0
        && collision.direct_sources.contains(&(move_.source.x,move_.source.y)))
        .unwrap_or_else(||panic!("missing ordinary original ValidPoint movement {from}->{to}"));
    let (start,direction)=collision.adjacent(&movement.source,from);
    let mut runtime=prepared_actor(config,class,from,start);
    let before=runtime.active_character_checkpoint().unwrap();
    let transfers=runtime.world_snapshot().map_transfers;
    let transfer=transfers.iter().find(|transfer|transfer.map_file_name==from
        && transfer.to_map_file_name==to && transfer.to_position==movement.destination
        && transfer.bounds.min_x==movement.source.x && transfer.bounds.min_y==movement.source.y)
        .unwrap_or_else(||panic!("runtime omits original {class:?} {from}->{to} movement"));
    assert!(transfer.key.starts_with("crystal-move:"),"only canonical authored movement key");

    // The gateway's automatic doorway path selects the first actual current
    // source bound. An overlapping earlier movement must not be bypassed by
    // directly choosing our preferred target key.
    let actual=transfers.iter().find(|transfer|transfer.map_file_name==from
        && movement.source.x>=transfer.bounds.min_x && movement.source.x<=transfer.bounds.max_x
        && movement.source.y>=transfer.bounds.min_y && movement.source.y<=transfer.bounds.max_y).unwrap();
    assert_eq!(actual.key,transfer.key,"automatic source selection {class:?} {from}->{to}");
    let key=actual.key.clone();

    let join=runtime.active_zone_join_snapshot(format!("doorway-{class:?}-{from}-{to}")).unwrap();
    let session_id=join.session_id.clone();
    let mut zone=ZoneRuntime::new_with_collision(ZoneKey::for_map(from),collision.collision.clone());
    zone.handle(ZoneCommand::Join(join));
    let mut outbound=zone.handle(ZoneCommand::Walk{session_id:session_id.clone(),direction,seq:1,now_ms:0});
    outbound.extend(zone.tick(0));
    let (committed,committed_direction)=outbound.iter().find_map(|item|match item {
        ZoneOutbound::SaveTransform{session_id:saved,position,direction} if saved==&session_id
            && position==&movement.source=>Some((position.clone(),*direction)),
        _=>None,
    }).unwrap_or_else(||panic!("original Zone Walk did not reach {class:?} {from}->{to} {outbound:?}"));
    assert!(outbound.iter().any(|item|match item {
        ZoneOutbound::ToSession{session_id:owner,packets} if owner==&session_id => packets.iter()
            .any(|p|matches!(p,ServerPacket::UserLocation{location} if location.position==committed)),
        _=>false,
    }),"owner must receive the committed original doorway coordinate");

    // This is the existing trusted gateway SaveTransform -> canonical transfer
    // bridge, not a client MoveTo/Stage5/debug relocation or source override.
    runtime.force_authoritative_player_transform(committed,committed_direction);
    let packets=runtime.execute(WorldCommand::TransferMap{key}).unwrap();
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::MapInformation{info}
        if info.file_name==to && info.map_index==target.map_index)),"destination identity {class:?} {from}->{to}");
    let after=runtime.active_character_checkpoint().unwrap();
    assert_eq!(after.map_file_name,to,"committed map {class:?} {from}->{to}");
    assert_eq!(after.position,movement.destination,"original destination {class:?} {from}->{to}");
    assert_eq!(after.character.class,class);
    assert_eq!(after.inventory_items_json,before.inventory_items_json,"doorways must not manufacture or debit loot");

    let logout=runtime.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)).unwrap();
    assert!(logout.iter().any(|p|matches!(p,ServerPacket::LogOutSuccess{..})));
    let stored=config.account_store.lock().unwrap().accounts["prepared-classic-doorway"].saves[&0].clone();
    assert_eq!(stored.map_file_name,to);
    assert_eq!(stored.position,movement.destination);
    let relog=runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame{character_index:0})).unwrap();
    assert!(relog.iter().any(|p|matches!(p,ServerPacket::MapInformation{info} if info.file_name==to)));
    assert_eq!(runtime.active_character_checkpoint().unwrap().position,movement.destination);
}

#[test]
fn three_classes_walk_all_76_authored_late_doorway_directions_and_save_relogin() {
    set_crystal_full_world_zone_collision(true);
    // Password hashing/config import is fixture setup, reused across cases.
    // Each doorway still starts a new authenticated runtime and has its own
    // explicitly prepared save; no counted movement is supplied by the fixture.
    let config=SimulationConfig::default().with_crystal_world_runtime().with_platinum_176_profile();
    let mut collisions=BTreeMap::new();
    for (_,route) in ROUTES {
        for map in &route[..route.len()-1] {
            collisions.entry(*map).or_insert_with(||SourceCollision::load(map));
        }
    }
    let mut count=0;
    for class in [MirClass::Warrior,MirClass::Wizard,MirClass::Taoist] {
        for (name,route) in ROUTES {
            for pair in route.windows(2) {
                run_doorway(&config,class,pair[0],pair[1],&collisions[pair[0]]);
                count+=1;
            }
            eprintln!("prepared doorway source {class:?} {name}: {} ordinary directions, saved/relogged",route.len()-1);
        }
    }
    assert_eq!(count,76*3);
}

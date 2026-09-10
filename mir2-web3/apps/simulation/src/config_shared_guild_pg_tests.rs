use super::*;
use std::sync::Barrier;
fn race(url:&str,plans:[AccountStoreMutationPlan;2])->[Result<AccountStoreSourceVersions,String>;2]{
    let mut clients=[Client::connect(url,NoTls).unwrap(),Client::connect(url,NoTls).unwrap()];
    for client in &mut clients {client.batch_execute("SET lock_timeout='3s'; SET statement_timeout='10s'").unwrap();}
    let barrier=Arc::new(Barrier::new(2));
    let handles=clients.into_iter().zip(plans).map(|(mut client,plan)|{let barrier=barrier.clone();std::thread::spawn(move||{barrier.wait();write_account_store_mutation_plan_to_postgres(&mut client,&plan,AccountStoreDatabaseMode::SourceOfTruth)})}).collect::<Vec<_>>();
    let mut outcomes=handles.into_iter().map(|handle|handle.join().unwrap());[outcomes.next().unwrap(),outcomes.next().unwrap()]
}
fn assert_single_winner(outcomes:&[Result<AccountStoreSourceVersions,String>;2]){
    assert_eq!(outcomes.iter().filter(|result|result.is_ok()).count(),1,"{outcomes:?}");
    let error=outcomes.iter().find_map(|result|result.as_ref().err()).unwrap();
    assert!(!error.contains("deadlock")&&!error.contains("timeout"),"{error}");
    assert!(error.contains("stale")||error.contains("duplicate"),"{error}");
}
#[test]
#[ignore="requires dedicated MIR2_GUILD_TEST_DATABASE_URL; two independent PostgreSQL connections"]
fn guild_postgres_concurrent_balance_and_cross_guild_membership_are_atomic(){
    let url=std::env::var("MIR2_GUILD_TEST_DATABASE_URL").expect("dedicated guild test database required");
    let mut client=Client::connect(&url,NoTls).unwrap();crate::db_projection::apply_migrations(&mut client).unwrap();
    let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let ids=[format!("{nonce:032x}"),format!("{:032x}",nonce+1)];
    let accounts=(0..3).map(|n|format!("guild-race-{nonce}-{n}")).collect::<Vec<_>>();
    let config=SimulationConfig::default();let mut empty=config.account_store.lock().unwrap().clone();empty.accounts.clear();
    let mut genesis=empty.clone();
    for (index,account_id) in accounts.iter().enumerate(){let character=CharacterRecord{index:index as i32,name:format!("Race{index}"),level:22,class:MirClass::Warrior,gender:MirGender::Male};let mut account=AccountRecord::new(character);account.saves.get_mut(&(index as i32)).unwrap().gold=100;genesis.accounts.insert(account_id.clone(),account);}
    for (index,id) in ids.iter().enumerate(){genesis.shared_guilds.insert(id.clone(),SharedGuildRecord{id:id.clone(),name:format!("R{index}{:x}",nonce%0xffffffffffffff),revision:1,level:0,experience:0,spare_points:0,gold:100,ranks:vec![SharedGuildRank{index:0,name:"Leader".into(),options:255},SharedGuildRank{index:1,name:"Members".into(),options:0}],members:vec![SharedGuildMember{ membership_epoch: 0,identity:Stage5FriendIdentity{account_id:accounts[index].clone(),character_index:index as i32},name:format!("Race{index}"),rank_index:0}],notice:vec![],storage:BTreeMap::new(),buffs:BTreeMap::new(),last_buff_tick_ms:0,experience_receipts:BTreeSet::new(),experience_receipt_payloads:Default::default()});}
    let initial=build_account_store_mutation_plan(&empty,&genesis,AccountStoreMutationScope::AccountsWithGuilds{account_ids:&accounts,guild_ids:&ids},false);
    let versions=write_account_store_mutation_plan_to_postgres(&mut client,&initial,AccountStoreDatabaseMode::SourceOfTruth).unwrap();apply_account_store_mutation_source_versions(&mut genesis,&initial,versions);
    let mut withdrawal=genesis.clone();withdrawal.shared_guilds.get_mut(&ids[0]).unwrap().gold=30;withdrawal.shared_guilds.get_mut(&ids[0]).unwrap().revision+=1;withdrawal.accounts.get_mut(&accounts[0]).unwrap().saves.get_mut(&0).unwrap().gold=170;
    let plan=build_account_store_mutation_plan(&genesis,&withdrawal,AccountStoreMutationScope::AccountsWithGuilds{account_ids:&accounts[..1],guild_ids:&ids[..1]},false);
    let outcomes=race(&url,[plan.clone(),plan]);assert_single_winner(&outcomes);
    let row=client.query_one("SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",&[&ids[0]]).unwrap();let guild:SharedGuildRecord=serde_json::from_value(row.get(0)).unwrap();assert_eq!(guild.gold,30);genesis.shared_guilds.insert(ids[0].clone(),guild);genesis.source_guild_versions.insert(ids[0].clone(),row.get(1));
    let row=client.query_one("SELECT raw_json,store_version FROM accounts WHERE account_id=$1",&[&accounts[0]]).unwrap();genesis.accounts.insert(accounts[0].clone(),serde_json::from_value(row.get(0)).unwrap());genesis.source_account_versions.insert(accounts[0].clone(),row.get(1));
    let row=client.query_one("SELECT gold,save_version FROM character_saves WHERE account_id=$1 AND character_index=0",&[&accounts[0]]).unwrap();assert_eq!(row.get::<_,i64>(0),170);genesis.source_save_versions.entry(accounts[0].clone()).or_default().insert(0,row.get(1));
    let plans=ids.iter().enumerate().map(|(index,id)|{let mut joined=genesis.clone();let guild=joined.shared_guilds.get_mut(id).unwrap();guild.members.push(SharedGuildMember{ membership_epoch: 0,identity:Stage5FriendIdentity{account_id:accounts[2].clone(),character_index:2},name:"Race2".into(),rank_index:1});guild.gold+=10;guild.revision+=1;joined.accounts.get_mut(&accounts[2]).unwrap().saves.get_mut(&2).unwrap().gold=90;let scoped=vec![accounts[index].clone(),accounts[2].clone()];build_account_store_mutation_plan(&genesis,&joined,AccountStoreMutationScope::AccountsWithGuilds{account_ids:&scoped,guild_ids:std::slice::from_ref(id)},false)}).collect::<Vec<_>>();
    let outcomes=race(&url,[plans[0].clone(),plans[1].clone()]);assert_single_winner(&outcomes);
    let row=client.query_one("SELECT count(*) FROM shared_guild_members WHERE account_id=$1 AND character_index=2",&[&accounts[2]]).unwrap();assert_eq!(row.get::<_,i64>(0),1);
    let row=client.query_one("SELECT gold FROM character_saves WHERE account_id=$1 AND character_index=2",&[&accounts[2]]).unwrap();assert_eq!(row.get::<_,i64>(0),90);
    let mut bank=0;for id in &ids{let row=client.query_one("SELECT raw_json FROM shared_guilds WHERE guild_id=$1",&[id]).unwrap();let guild:SharedGuildRecord=serde_json::from_value(row.get(0)).unwrap();bank+=guild.gold;}assert_eq!(bank,140,"the losing guild must not retain a debit or member");
    for id in &ids{client.execute("DELETE FROM shared_guilds WHERE guild_id=$1",&[id]).unwrap();}for account in &accounts{client.execute("DELETE FROM accounts WHERE account_id=$1",&[account]).unwrap();}
}

from pathlib import Path
import hashlib, json
root=Path(__file__).resolve().parents[2]
project=root/'mir2-web3'
before=Path(__file__).parent/'before-authority'
before.mkdir(exist_ok=True)
rows=[]
def edit(rel, changes):
    path=project/rel
    current=path.read_bytes()
    dest=before/rel
    dest.parent.mkdir(parents=True,exist_ok=True)
    original=dest.read_bytes() if dest.exists() else current
    if not dest.exists(): dest.write_bytes(original)
    text=original.decode('utf-8')
    for old,new,count in changes:
        if text.count(old)!=count:
            alt=old.replace('\n','\r\n')
            if text.count(alt)!=count: raise RuntimeError((rel,old[:100],text.count(old),text.count(alt),count))
            old,new=alt,new.replace('\n','\r\n')
        text=text.replace(old,new)
    changed=text.encode('utf-8')
    if current not in (original,changed): raise RuntimeError('unexpected concurrent drift '+rel)
    path.write_bytes(changed)
    rows.append({'path':'mir2-web3/'+rel,'beforeSha256':hashlib.sha256(original).hexdigest(),'afterSha256':hashlib.sha256(path.read_bytes()).hexdigest()})
edit('packages/production/src/lib.rs', [
    ('mod planning;','mod planning;\nmod stones;\npub use stones::*;',1),
])
edit('apps/simulation/src/config.rs', [
 ('const ACCOUNT_STORE_SCHEMA_VERSION: u16 = 5;', '#[path = "config_stones.rs"]\nmod stone_authority;\nconst ACCOUNT_STORE_SCHEMA_VERSION: u16 = 6;',1),
 ('pub struct AccountStore {','pub struct AccountStore {\n    /// Private results and cut tombstones. Never included in a client snapshot.\n    #[serde(rename="sealedStones", default)]\n    pub(crate) sealed_stones: mir2_production::StoneRegistry,\n    #[serde(skip)]\n    source_stone_version: Option<i64>,',1),
 ('shared_heroes: BTreeMap::new(),','shared_heroes: BTreeMap::new(),\n            sealed_stones: mir2_production::StoneRegistry::default(),\n            source_stone_version: None,',3),
 ('shared_heroes: self.shared_heroes.clone(),','shared_heroes: self.shared_heroes.clone(),\n            sealed_stones: self.sealed_stones.clone(),\n            source_stone_version: self.source_stone_version,',1),
 ('let impossible_guilds=self.schema_version<3', 'let impossible_stones=self.schema_version<6 && self.sealed_stones!=mir2_production::StoneRegistry::default();\n        let impossible_guilds=self.schema_version<3',1),
 ('&& !impossible_heroes {','&& !impossible_heroes && !impossible_stones {',1),
 ('self.source_hero_versions = versions.heroes.heroes;','if let Some(version)=versions.stones {self.source_stone_version=Some(version);}\n        self.source_hero_versions = versions.heroes.heroes;',1),
 ('self.source_hero_versions.extend(versions.heroes.heroes);','if let Some(version)=versions.stones {self.source_stone_version=Some(version);}\n        self.source_hero_versions.extend(versions.heroes.heroes);',1),
 ('store.source_hero_versions.extend(versions.heroes.heroes);','if let Some(version)=versions.stones {store.source_stone_version=Some(version);}\n    store.source_hero_versions.extend(versions.heroes.heroes);',1),
 ('struct AccountStoreSourceVersions {','struct AccountStoreSourceVersions {\n    stones: Option<i64>,',1),
 ("enum AccountStoreMutationScope<'a> {", "enum AccountStoreMutationScope<'a> {\n    AccountsWithStones { account_ids: &'a [String], stone_ids: &'a [u64] },\n    AccountsWithStonesAndGuilds { account_ids: &'a [String], stone_ids: &'a [u64], guild_ids: &'a [String] },",1),
 ('InvalidHeroState(String),','InvalidHeroState(String),\n    InvalidStoneState(String),',1),
 ('Self::InvalidHeroState(reason) =>', 'Self::InvalidStoneState(reason) => write!(formatter, "invalid sealed stone state: {reason}"),\n            Self::InvalidHeroState(reason) =>',1),
 ('hero_registry::validate_scope(original, staged, scope)?;','stone_authority::validate_scope(original, staged, scope)?;\n    hero_registry::validate_scope(original, staged, scope)?;',1),
 ('| AccountStoreMutationScope::AccountsWithHeroesAndGuilds { account_ids, .. } => (account_ids, false),','| AccountStoreMutationScope::AccountsWithHeroesAndGuilds { account_ids, .. }\n        | AccountStoreMutationScope::AccountsWithStones { account_ids, .. }\n        | AccountStoreMutationScope::AccountsWithStonesAndGuilds { account_ids, .. } => (account_ids, false),',1),
 ('struct AccountStoreMutationPlan {','struct AccountStoreMutationPlan {\n    stones: Option<stone_authority::StoneMutation>,',1),
 ('| AccountStoreMutationScope::AccountsWithGlobalAndGuilds { account_ids: scoped_ids, .. } => {','| AccountStoreMutationScope::AccountsWithGlobalAndGuilds { account_ids: scoped_ids, .. }\n        | AccountStoreMutationScope::AccountsWithStones { account_ids: scoped_ids, .. }\n        | AccountStoreMutationScope::AccountsWithStonesAndGuilds { account_ids: scoped_ids, .. } => {',1),
 ('AccountStoreMutationPlan {\n        force_source_cas:', 'AccountStoreMutationPlan {\n        stones: stone_authority::build(original, desired),\n        force_source_cas:',1),
 ('pub struct AccountStoreRepositorySave {','pub struct AccountStoreRepositorySave {\n    pub(crate) stones: Option<i64>,',1),
 ('clock: value.clock,','stones: value.stones,\n            clock: value.clock,',1),
 ('clock: self.clock,','stones: self.stones,\n            clock: self.clock,',1),
 ('hero_registry::validate_complete_state(&store)?;','stone_authority::validate_store(&store)?;\n        hero_registry::validate_complete_state(&store)?;',2),
 ('hero_registry::validate_complete_state(&decoded)?;','stone_authority::validate_store(&decoded)?;\n        hero_registry::validate_complete_state(&decoded)?;',1),
 ('hero_registry::validate_complete_state(&staged_store)?;','stone_authority::validate_store(&staged_store)?;\n            hero_registry::validate_complete_state(&staged_store)?;',1),
 ('restored.source_hero_versions.clear();','restored.source_stone_version=None;\n        restored.source_hero_versions.clear();',1),
 ('hero_postgres::load_heroes(&mut transaction, &mut store)?;','stone_authority::load(&mut transaction, &mut store)?;\n        hero_postgres::load_heroes(&mut transaction, &mut store)?;',1),
 ('pub(crate) fn commit_account_store_transaction_with_global<T, F>(', '''pub(crate) fn commit_account_store_transaction_with_stones<T, F>(
        &self, account_ids: &[String], stone_ids: &[u64], transaction: F,
    ) -> Result<T, String> where F: FnOnce(&mut AccountStore)->Result<T,String> {
        if account_ids.is_empty() || stone_ids.len()!=1 || stone_ids[0]==0 {
            return Err("stone transaction requires one exact identity and an authenticated account".into());
        }
        // There is no business rollback of an issued sealed result. File with
        // an independent PostgreSQL mirror therefore cannot run this producer.
        if self.account_store_path.is_some() && self.account_store_database_url.is_some() {
            return Err("stone authority requires one durable source, not File plus Mirror".into());
        }
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::AccountsWithStones { account_ids, stone_ids }, transaction)
    }

    pub(crate) fn commit_account_store_transaction_with_global<T, F>(''',1),
 ('| AccountStoreMutationScope::AccountsWithGlobal(account_ids) => account_ids.is_empty(),','| AccountStoreMutationScope::AccountsWithGlobal(account_ids)\n            | AccountStoreMutationScope::AccountsWithStones { account_ids, .. }\n            | AccountStoreMutationScope::AccountsWithStonesAndGuilds { account_ids, .. } => account_ids.is_empty(),',1),
 ('| AccountStoreMutationScope::AccountsWithHeroesAndGuilds { account_ids, .. } => account_ids,','| AccountStoreMutationScope::AccountsWithHeroesAndGuilds { account_ids, .. }\n            | AccountStoreMutationScope::AccountsWithStones { account_ids, .. }\n            | AccountStoreMutationScope::AccountsWithStonesAndGuilds { account_ids, .. } => account_ids,',1),
 ('| AccountStoreMutationScope::AccountsWithHeroesAndGuilds { guild_ids, .. } = scope {','| AccountStoreMutationScope::AccountsWithHeroesAndGuilds { guild_ids, .. }\n            | AccountStoreMutationScope::AccountsWithStonesAndGuilds { guild_ids, .. } = scope {',1),
 ('_ => AccountStoreMutationScope::AccountsWithGuilds { account_ids: xp_accounts, guild_ids: &xp_guild_ids },','AccountStoreMutationScope::AccountsWithStones { stone_ids, .. } | AccountStoreMutationScope::AccountsWithStonesAndGuilds { stone_ids, .. } => AccountStoreMutationScope::AccountsWithStonesAndGuilds { account_ids: xp_accounts, stone_ids, guild_ids: &xp_guild_ids },\n                _ => AccountStoreMutationScope::AccountsWithGuilds { account_ids: xp_accounts, guild_ids: &xp_guild_ids },',1),
 ('AccountStoreMutationScope::AccountsWithGuilds { .. } => {','AccountStoreMutationScope::AccountsWithStones { .. } | AccountStoreMutationScope::AccountsWithStonesAndGuilds { .. } => AccountStoreTransactionScopeObservation::WithGlobal,\n            AccountStoreMutationScope::AccountsWithGuilds { .. } => {',1),
 ('staged_store.source_hero_versions.extend(versions.heroes.heroes.clone());','if let Some(version)=versions.stones {staged_store.source_stone_version=Some(version);}\n            staged_store.source_hero_versions.extend(versions.heroes.heroes.clone());',1),
 ('|| ((!plan.heroes.is_empty() || plan.hero_allocator.is_some()) && error.starts_with(hero_registry::HERO_COMMIT_OUTCOME_UNKNOWN)) {','|| ((!plan.heroes.is_empty() || plan.hero_allocator.is_some()) && error.starts_with(hero_registry::HERO_COMMIT_OUTCOME_UNKNOWN))\n            || (plan.stones.is_some() && error.starts_with(stone_authority::COMMIT_UNKNOWN)) {',1),
 ('let carries_heroes=!plan.heroes.is_empty() || plan.hero_allocator.is_some();','let carries_stones=plan.stones.is_some();\n    let carries_heroes=!plan.heroes.is_empty() || plan.hero_allocator.is_some();',1),
 ('else if carries_heroes{format!', 'else if carries_stones{format!("{}: stone transaction worker panicked",stone_authority::COMMIT_UNKNOWN)}else if carries_heroes{format!',1),
 ('else if (!plan.heroes.is_empty() || plan.hero_allocator.is_some()) && error.as_db_error().is_none(){','else if plan.stones.is_some() && error.as_db_error().is_none(){format!("{}: stone commit response unavailable: {error}",stone_authority::COMMIT_UNKNOWN)}else if (!plan.heroes.is_empty() || plan.hero_allocator.is_some()) && error.as_db_error().is_none(){',1),
 ('let hero_versions=hero_postgres::write_hero_mutations(', 'let stone_version=stone_authority::write(transaction,plan.stones.as_ref())?;\n    let hero_versions=hero_postgres::write_hero_mutations(',1),
 ('source_versions.clock=clock_version;','source_versions.stones=stone_version;\n    source_versions.clock=clock_version;',1),
])
edit('apps/simulation/src/db_projection.rs', [
    ('pub const MIGRATIONS: &[(&str, &str)] = &[', 'pub const MIGRATIONS: &[(&str, &str)] = &[',1),
    ('"0014_shared_hero_authority",\n        include_str!("../../../infra/postgres/migrations/0014_shared_hero_authority.sql"),\n    ),','"0014_shared_hero_authority",\n        include_str!("../../../infra/postgres/migrations/0014_shared_hero_authority.sql"),\n    ),\n    ("0015_sealed_stone_authority", include_str!("../../../infra/postgres/migrations/0015_sealed_stone_authority.sql")),',1),
])
(Path(__file__).parent/'authority-edit-receipt.json').write_text(json.dumps({'files':rows},indent=2),encoding='utf-8')
print(json.dumps(rows,indent=2))

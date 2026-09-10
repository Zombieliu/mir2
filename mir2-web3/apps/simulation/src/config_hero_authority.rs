//! Reconcile the independent Hero source before issuing any live attachment grant.
//! A File primary can recover source versions only from identical business data;
//! a divergent mirror is not permission to replace either authority silently.
use super::*;
use super::super::{SimulationConfig,AccountStoreDatabaseMode,load_account_store_from_postgres};

impl SimulationConfig {
    pub(crate) fn refresh_shared_hero_authority(&self)->Result<(),String>{
        let _persist=self.account_store_persist_lock.lock().map_err(|_|"account store persist mutex poisoned")?;
        self.ensure_account_store_writable()?;
        self.ensure_file_writer_binding()?;
        let Some(url)=self.account_store_database_url.as_ref() else {return Ok(());};
        let loaded=load_account_store_from_postgres(url.clone(),self.default_character.clone())?;
        validate_complete_state(&loaded)?;
        let mut live=self.account_store.lock().map_err(|_|"account store mutex poisoned")?;
        if self.account_store_database_mode==AccountStoreDatabaseMode::Mirror {
            if live.shared_heroes!=loaded.shared_heroes || live.hero_id_high_watermark!=loaded.hero_id_high_watermark {
                return Err(self.freeze_account_store_writes("Hero File primary and PostgreSQL mirror disagree; reconciliation required".into()));
            }
        }else{
            live.shared_heroes=loaded.shared_heroes;
            live.hero_id_high_watermark=loaded.hero_id_high_watermark;
        }
        live.source_hero_versions=loaded.source_hero_versions;
        live.source_hero_allocator_version=loaded.source_hero_allocator_version;
        Ok(())
    }
}

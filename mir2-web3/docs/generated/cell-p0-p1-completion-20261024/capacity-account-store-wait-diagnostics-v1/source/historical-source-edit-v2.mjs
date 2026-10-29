import fs from 'node:fs'; import p from 'node:path'; import c from 'node:crypto';
const root='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/apps/simulation/src';
const history='C:/mir2-build/p01-completion-20261024/formal12-account-store-diagnostics-v1';
const pre=fs.readFileSync(p.join(root,'config_guild_clock_driver.rs'));fs.writeFileSync(p.join(history,'preimages','config_guild_clock_driver.rs.txt'),pre,{flag:'wx'});
function edit(file,fn){const target=p.join(root,file),before=fs.readFileSync(target,'utf8'),nl=before.includes('\r\n')?'\r\n':'\n';const n=s=>s.replace(/\r?\n/g,nl);let s=before;function one(a,b){a=n(a);b=n(b);if(s.split(a).length!==2)throw Error(file+' expected one '+a.slice(0,100));s=s.replace(a,b);}function scope(start,end,change){const i=s.indexOf(n(start)),j=s.indexOf(n(end),i+start.length);if(i<0||j<0)throw Error('scope '+start);const body=s.slice(i,j);s=s.slice(0,i)+change(body,n)+s.slice(j);}fn({one,scope,n});fs.writeFileSync(target,s);return{file,beforeBytes:Buffer.byteLength(before),afterBytes:Buffer.byteLength(s),sha256:c.createHash('sha256').update(s).digest('hex')}}
const output=[];
output.push(edit('lib.rs',({one})=>one('mod config;','mod config;\nmod config_account_store_diagnostics;')));
output.push(edit('config.rs',({one,scope,n})=>{
 one('use crate::conquest::{ConquestPolicy, SharedConquestRecord};','use crate::conquest::{ConquestPolicy, SharedConquestRecord};\nuse crate::config_account_store_diagnostics::{\n    measure as diagnostic_measure, Operation as DiagnosticOperation, Record as DiagnosticRecord,\n    Stage as DiagnosticStage,\n};');
 one(`    fn save_to_path_with_fault(
        &self,
        path: &Path,
        fault: Option<AccountStoreFileCommitFault>,
    ) -> Result<(), AccountStoreFileCommitError> {
        if let Some(parent) = path.parent() {`,`    fn save_to_path_with_fault(
        &self,
        path: &Path,
        fault: Option<AccountStoreFileCommitFault>,
    ) -> Result<(), AccountStoreFileCommitError> {
        self.save_to_path_with_diagnostics(path, fault, None)
    }

    fn save_to_path_with_diagnostics(
        &self,
        path: &Path,
        fault: Option<AccountStoreFileCommitFault>,
        diagnostics: Option<&DiagnosticRecord>,
    ) -> Result<(), AccountStoreFileCommitError> {
        if let Some(parent) = path.parent() {`);
 one(`        let data = serde_json::to_string_pretty(self).map_err(|error| {`,`        let data = diagnostic_measure(diagnostics, DiagnosticStage::Serialize, || {
            serde_json::to_string_pretty(self)
        }).map_err(|error| {`);
 one('        write_file_atomically(path, data.as_bytes(), fault).map_err(|error| {','        write_file_atomically_with_diagnostics(path, data.as_bytes(), fault, diagnostics).map_err(|error| {');
 one(`        save_account_store_snapshot_to_path(store, &self.path, fault)?;`,`        self.save_with_commit_outcome_and_diagnostics(store, fault, None)
    }

    fn save_with_commit_outcome_and_diagnostics(
        &self,
        store: &AccountStore,
        fault: Option<AccountStoreFileCommitFault>,
        diagnostics: Option<&DiagnosticRecord>,
    ) -> Result<AccountStoreRepositorySave, AccountStoreFileCommitError> {
        save_account_store_snapshot_to_path(store, &self.path, fault, diagnostics)?;`);
 one(`) -> Result<(), AccountStoreFileCommitError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));`,`) -> Result<(), AccountStoreFileCommitError> {
    write_file_atomically_with_diagnostics(path, data, fault, None)
}

fn write_file_atomically_with_diagnostics(
    path: &Path,
    data: &[u8],
    fault: Option<AccountStoreFileCommitFault>,
    diagnostics: Option<&DiagnosticRecord>,
) -> Result<(), AccountStoreFileCommitError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));`);
 one('    let prepare_result = (|| -> io::Result<()> {','    let prepare_result = diagnostic_measure(diagnostics, DiagnosticStage::TempWriteSync, || -> io::Result<()> {');
 one('    match replace_file_atomically_with_retry(&temp_path, path, fault) {','    match diagnostic_measure(diagnostics, DiagnosticStage::AtomicReplace, || {\n        replace_file_atomically_with_retry(&temp_path, path, fault)\n    }) {');
 one(`    pub(crate) fn ensure_account_store_writable(&self) -> Result<(), String> {
        let state = self.account_store_write_state.lock().map_err(|_| {`,`    pub(crate) fn ensure_account_store_writable(&self) -> Result<(), String> {
        self.ensure_account_store_writable_with_diagnostics(None)
    }

    pub(crate) fn ensure_account_store_writable_with_diagnostics(
        &self,
        diagnostics: Option<&DiagnosticRecord>,
    ) -> Result<(), String> {
        let state = diagnostic_measure(diagnostics, DiagnosticStage::WritableWait, || {
            self.account_store_write_state.lock()
        }).map_err(|_| {`);
 one(`    fn begin_file_publication(
        &self,
    ) -> Result<Option<file_authority::PublicationGuard<'_>>, String> {
        match self.account_store_path.as_deref() {`,`    fn begin_file_publication(
        &self,
    ) -> Result<Option<file_authority::PublicationGuard<'_>>, String> {
        self.begin_file_publication_with_diagnostics(None)
    }

    fn begin_file_publication_with_diagnostics(
        &self,
        diagnostics: Option<&DiagnosticRecord>,
    ) -> Result<Option<file_authority::PublicationGuard<'_>>, String> {
        match self.account_store_path.as_deref() {`);
 one('                authority.begin_publication().map(Some)','                authority.begin_publication_with_diagnostics(diagnostics).map(Some)');
 one(`    fn save_file_account_store(
        &self,
        path: &Path,
        store: &AccountStore,
    ) -> Result<AccountStoreRepositorySave, AccountStoreFileCommitError> {
        if !self`,`    fn save_file_account_store(
        &self,
        path: &Path,
        store: &AccountStore,
    ) -> Result<AccountStoreRepositorySave, AccountStoreFileCommitError> {
        self.save_file_account_store_with_diagnostics(path, store, None)
    }

    fn save_file_account_store_with_diagnostics(
        &self,
        path: &Path,
        store: &AccountStore,
        diagnostics: Option<&DiagnosticRecord>,
    ) -> Result<AccountStoreRepositorySave, AccountStoreFileCommitError> {
        if !self`);
 one('        FileAccountStoreRepository::new(path).save_with_commit_outcome(store, fault)','        FileAccountStoreRepository::new(path).save_with_commit_outcome_and_diagnostics(store, fault, diagnostics)');
 scope('    fn save_account_store_with_plan_writer<F>(', '    /// Commit a bounded multi-account mutation', (s)=>{
  const replace=(a,b)=>{a=n(a);b=n(b);if(s.split(a).length!==2)throw Error('save '+a);s=s.replace(a,b)};
  replace('        let _persist_guard = self','        let diagnostics = DiagnosticRecord::new(DiagnosticOperation::Save);\n        let persist_wait = diagnostics.span(DiagnosticStage::PersistWait);\n        let _persist_guard = self');
  replace('        self.ensure_account_store_writable()?;','        drop(persist_wait);\n        self.ensure_account_store_writable_with_diagnostics(Some(&diagnostics))?;');
  replace('        let store = {\n            let store = self','        let store = {\n            let store_wait = diagnostics.span(DiagnosticStage::StoreWait);\n            let store = self');
  replace('            store.clone()','            drop(store_wait);\n            let _store_held = diagnostics.span(DiagnosticStage::StoreHeld);\n            store.clone()');
  s=s.replaceAll('self.begin_file_publication()?','self.begin_file_publication_with_diagnostics(Some(&diagnostics))?').replaceAll('self.save_file_account_store(path, &store)','self.save_file_account_store_with_diagnostics(path, &store, Some(&diagnostics))').replaceAll('publication.settle()?','publication.settle_with_diagnostics(Some(&diagnostics))?');
  return s;
 });
 scope('    fn commit_account_store_transaction_authorized_post_inner<T, F, P>(', '    #[cfg(any(test, feature = "test-support"))]\n    pub fn inject_account_store_transaction_fault', (s)=>{
  const replace=(a,b)=>{a=n(a);b=n(b);if(s.split(a).length!==2)throw Error('txn '+a);s=s.replace(a,b)};
  replace('        let _persist_guard = self','        let diagnostics = DiagnosticRecord::new(DiagnosticOperation::Transaction);\n        let persist_wait = diagnostics.span(DiagnosticStage::PersistWait);\n        let _persist_guard = self');
  replace('        self.ensure_account_store_writable()?;','        drop(persist_wait);\n        self.ensure_account_store_writable_with_diagnostics(Some(&diagnostics))?;');
  replace('        let mut live_store = self','        let store_wait = diagnostics.span(DiagnosticStage::StoreWait);\n        let mut live_store = self');
  replace('        let original_store = live_store.clone();','        drop(store_wait);\n        let _store_held = diagnostics.span(DiagnosticStage::StoreHeld);\n        let original_store = live_store.clone();');
  s=s.replaceAll('self.begin_file_publication()?','self.begin_file_publication_with_diagnostics(Some(&diagnostics))?').replaceAll('publication.settle()?','publication.settle_with_diagnostics(Some(&diagnostics))?');
  for(const value of ['&staged_store','&hero_restored','&restored']){s=s.replaceAll(`self.save_file_account_store(path, ${value})`,`self.save_file_account_store_with_diagnostics(path, ${value}, Some(&diagnostics))`).replaceAll(`self.save_file_account_store(path,${value})`,`self.save_file_account_store_with_diagnostics(path,${value},Some(&diagnostics))`)}
  return s;
 });
 one(`    fault: Option<AccountStoreFileCommitFault>,
) -> Result<(), AccountStoreFileCommitError> {
    let _guard = ACCOUNT_STORE_FILE_WRITE_LOCK`,`    fault: Option<AccountStoreFileCommitFault>,
    diagnostics: Option<&DiagnosticRecord>,
) -> Result<(), AccountStoreFileCommitError> {
    let writer_wait = diagnostics.map(|record| record.span(DiagnosticStage::FileWriterWait));
    let _guard = ACCOUNT_STORE_FILE_WRITE_LOCK`);
 one('    store.save_to_path_with_fault(path, fault)','    drop(writer_wait);\n    store.save_to_path_with_diagnostics(path, fault, diagnostics)');
}));
output.push(edit('runtime/shared_experience_profile.rs',({one})=>{
 one('use crate::{AccountStore, SimulationConfig, Stage5FriendIdentity, Stage5SystemsState};','use crate::{AccountStore, SimulationConfig, Stage5FriendIdentity, Stage5SystemsState};\nuse crate::config_account_store_diagnostics::{Operation, Record, Stage};');
 one(`    ) -> Result<T, String> {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "experience profile authority mutex poisoned")?;
        self.ensure_account_store_writable()?;`,`    ) -> Result<T, String> {
        let diagnostics = Record::new(Operation::ExperienceAdmission);
        self.ensure_account_store_writable_with_diagnostics(Some(&diagnostics))?;
        let store_wait = diagnostics.span(Stage::StoreWait);
        let store = self
            .account_store
            .lock()
            .map_err(|_| "experience profile authority mutex poisoned")?;
        drop(store_wait);
        let _store_held = diagnostics.span(Stage::StoreHeld);
        self.ensure_account_store_writable_with_diagnostics(Some(&diagnostics))?;`);
}));
output.push(edit('config_shared_guilds.rs',({one})=>{
 one(`    ) -> Result<Option<SharedGuildRecord>, String> {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "guild authority mutex poisoned")?;
        // Clone only the owner's record`,`    ) -> Result<Option<SharedGuildRecord>, String> {
        let diagnostics = DiagnosticRecord::new(DiagnosticOperation::GuildLookup);
        self.ensure_account_store_writable_with_diagnostics(Some(&diagnostics))?;
        let store_wait = diagnostics.span(DiagnosticStage::StoreWait);
        let store = self
            .account_store
            .lock()
            .map_err(|_| "guild authority mutex poisoned")?;
        drop(store_wait);
        let _store_held = diagnostics.span(DiagnosticStage::StoreHeld);
        // Clone only the owner's record`);
}));
output.push(edit('config_file_authority.rs',({one})=>{
 one('    pub fn settle(mut self) -> Result<(), String> {','    pub fn settle(self) -> Result<(), String> {\n        self.settle_with_diagnostics(None)\n    }\n\n    pub(super) fn settle_with_diagnostics(\n        mut self,\n        diagnostics: Option<&DiagnosticRecord>,\n    ) -> Result<(), String> {\n        let marker_wait = diagnostics.map(|record| record.span(DiagnosticStage::MarkerWait));');
 one('        let state = self\n            .authority','        drop(marker_wait);\n        let state_wait = diagnostics.map(|record| record.span(DiagnosticStage::MarkerStateWait));\n        let state = self\n            .authority');
 one('        if let AccountStoreWriteState::Frozen { reason } = &*state {','        drop(state_wait);\n        if let AccountStoreWriteState::Frozen { reason } = &*state {');
 one('        if let Err(error) = self\n            .authority','        let _settle_sync = diagnostics.map(|record| record.span(DiagnosticStage::MarkerSettleSync));\n        if let Err(error) = self\n            .authority');
 one('    pub fn begin_publication(&self) -> Result<PublicationGuard<\'_>, String> {','    pub fn begin_publication(&self) -> Result<PublicationGuard<\'_>, String> {\n        self.begin_publication_with_diagnostics(None)\n    }\n\n    pub(super) fn begin_publication_with_diagnostics(\n        &self,\n        diagnostics: Option<&DiagnosticRecord>,\n    ) -> Result<PublicationGuard<\'_>, String> {');
 one(`        use std::io::{Seek, SeekFrom, Write};
        let _marker = self
            .marker_lock
            .lock()
            .map_err(|_| "publication marker mutex poisoned")?;
        if let AccountStoreWriteState::Frozen { reason } = &*self
            .write_state
            .lock()
            .map_err(|_| "publication write state poisoned")?`,`        use std::io::{Seek, SeekFrom, Write};
        let marker_wait = diagnostics.map(|record| record.span(DiagnosticStage::MarkerWait));
        let _marker = self
            .marker_lock
            .lock()
            .map_err(|_| "publication marker mutex poisoned")?;
        drop(marker_wait);
        if let AccountStoreWriteState::Frozen { reason } = &*diagnostic_measure(
            diagnostics, DiagnosticStage::MarkerStateWait, || self.write_state.lock(),
        ).map_err(|_| "publication write state poisoned")?`);
 one('        let mut file = &self._writer_lock;\n        file.seek(SeekFrom::Start(0))','        let _begin_sync = diagnostics.map(|record| record.span(DiagnosticStage::MarkerBeginSync));\n        let mut file = &self._writer_lock;\n        file.seek(SeekFrom::Start(0))');
}));
output.push(edit('config_guild_clock_driver.rs',({one,scope,n})=>{
 scope('    fn tick_shared_guild_clock_at(', '\n}\n\nstruct ClockObservation', s=>{
  const replace=(a,b)=>{a=n(a);b=n(b);if(s.split(a).length!==2)throw Error('clock '+a);s=s.replace(a,b)};
  replace('        let mut driver = self','        let diagnostics = DiagnosticRecord::new(DiagnosticOperation::GuildClock);\n        let driver_wait = diagnostics.span(DiagnosticStage::DriverWait);\n        let mut driver = self');
  replace('        if let Some(error) = &driver.frozen {','        drop(driver_wait);\n        let _driver_held = diagnostics.span(DiagnosticStage::DriverHeld);\n        if let Some(error) = &driver.frozen {');
  s=s.replaceAll('self.ensure_account_store_writable()?','self.ensure_account_store_writable_with_diagnostics(Some(&diagnostics))?');
  replace('        let _persist = self','        let persist_wait = diagnostics.span(DiagnosticStage::PersistWait);\n        let _persist = self');
  replace('            .map_err(|_| "account store persist mutex poisoned")?;','            .map_err(|_| "account store persist mutex poisoned")?;\n        drop(persist_wait);');
  replace('        let mut live = self','        let store_wait = diagnostics.span(DiagnosticStage::StoreWait);\n        let mut live = self');
  replace('        if driver.observed_guild_versions.is_none() {','        drop(store_wait);\n        let _store_held = diagnostics.span(DiagnosticStage::StoreHeld);\n        if driver.observed_guild_versions.is_none() {');
  s=s.replaceAll('self.begin_file_publication()?','self.begin_file_publication_with_diagnostics(Some(&diagnostics))?').replaceAll('publication.settle()?','publication.settle_with_diagnostics(Some(&diagnostics))?');
  for(const value of ['&staged','&restored'])s=s.replaceAll(`self.save_file_account_store(path, ${value})`,`self.save_file_account_store_with_diagnostics(path, ${value}, Some(&diagnostics))`);
  return s;
 });
}));
fs.writeFileSync(p.join(history,'source-edit-application-v1.json'),JSON.stringify({schemaVersion:1,rootAuthorizedExistingFiles:6,newModule:'config_account_store_diagnostics.rs',files:output},null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(output));

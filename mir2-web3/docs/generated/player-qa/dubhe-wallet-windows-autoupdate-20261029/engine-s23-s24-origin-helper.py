#!/usr/bin/env python3
"""Fixed 23/24 engine-only origin transaction. No game payload/service writes."""
import datetime, hashlib, json, os, pathlib, re, runpy, stat, sys, types, uuid

CODE_DIR = pathlib.Path('/var/lib/mir2-client-publisher')
ROOT = pathlib.Path('/srv/mir2-client-updates')
CADDY = pathlib.Path('/etc/caddy/Caddyfile')
BACKUPS = pathlib.Path('/var/backups/mir2-client-updates')
GAME_META = {'PACKAGE-MANIFEST.json','VERSION.json','RELEASE-STATEMENT.json','RELEASE-STATEMENT.p7s'}
ENGINE_FILES = {'ENGINE.json','ENGINE.p7s','Mir2Updater.exe'}

def need(value, code):
    if not value:
        raise ValueError(code)

def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=True).encode('ascii')

def fingerprint(value):
    return (value.st_dev,value.st_ino,value.st_size,value.st_mtime_ns,value.st_ctime_ns)

def plain(path, directory=False, root_owned=False, private_uid=None):
    value = pathlib.Path(path).lstat()
    need(not stat.S_ISLNK(value.st_mode) and (stat.S_ISDIR(value.st_mode) if directory else stat.S_ISREG(value.st_mode)), 'plain_path_required')
    if not directory:
        need(value.st_nlink == 1,'single_link_file_required')
    if root_owned:
        need(value.st_uid == 0 and not value.st_mode & 0o022,'root_owned_nonshared_path_required')
    if private_uid is not None:
        need(value.st_uid == private_uid and not value.st_mode & 0o077,'private_upload_path_required')
    return value

def ancestors(path, root_owned=False):
    path = pathlib.Path(path)
    need(path.is_absolute(),'absolute_path_required')
    for item in reversed((path,*path.parents)):
        plain(item,directory=True,root_owned=root_owned)
    return path

def checked_bytes(path, spec, limit, root_owned=False, private_uid=None):
    path = pathlib.Path(path)
    ancestors(path.parent,root_owned=root_owned)
    before = plain(path,root_owned=root_owned,private_uid=private_uid)
    need(type(spec['size']) is int and 0 < spec['size'] <= limit and before.st_size == spec['size'],'fixed_input_size_mismatch')
    need(re.fullmatch('[0-9A-F]{64}',spec['sha256']) is not None,'fixed_sha256_required')
    descriptor = os.open(path,os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor,'rb') as stream:
        opened = os.fstat(stream.fileno())
        need(fingerprint(before) == fingerprint(opened) and opened.st_nlink == 1,'input_changed_during_open')
        value = stream.read(spec['size']+1)
        need(fingerprint(opened) == fingerprint(os.fstat(stream.fileno())),'input_changed_during_snapshot')
    need(len(value) == spec['size'] and hashlib.sha256(value).hexdigest().upper() == spec['sha256'],'fixed_input_sha256_mismatch')
    return value

def write_fresh(path, data, mode=0o600):
    path = pathlib.Path(path)
    ancestors(path.parent,root_owned=True)
    descriptor = os.open(path,os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,mode)
    with os.fdopen(descriptor,'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    os.chmod(path,mode)
    sync_directory(path.parent)

def sync_directory(path):
    descriptor = os.open(path,os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)

def private_directory(path):
    path = pathlib.Path(path)
    ancestors(path.parent,root_owned=True)
    path.mkdir(mode=0o700)
    plain(path,directory=True,root_owned=True)
    sync_directory(path.parent)
    return path

def load_reviewed_code(spec, name):
    path = CODE_DIR / (spec['sha256']+'.py')
    need(str(path) == spec['path'],'fixed_root_code_path_required')
    checked_bytes(path,spec,65536,root_owned=True)
    # Only the SHA-pinned root-owned copy is executed. Upload directories never
    # appear in sys.path and are never passed to runpy/importlib.
    return types.SimpleNamespace(**runpy.run_path(str(path),run_name=name))

def feed_shape(feed, sequence, game_identity):
    need(set(feed) == {'schema','channel','platform','sequence','createdUnix','expiresUnix','minBootstrap','protocol','content','game','engine'},'feed_fields_differ')
    need(feed['schema'] == 'mir2.windows.update-feed.v1' and feed['channel'] == 'invited' and feed['platform'] == 'windows-x64'
         and type(feed['sequence']) is int and feed['sequence'] == sequence and type(feed['minBootstrap']) is int and feed['minBootstrap'] == 1
         and feed['protocol'] == 'crystal-mir2-v1' and feed['content'] == 'mir2.windows.package-manifest.v4','fixed_feed_identity_mismatch')
    now = int(datetime.datetime.now(datetime.timezone.utc).timestamp())
    need(type(feed['createdUnix']) is int and type(feed['expiresUnix']) is int
         and 0 < feed['createdUnix'] <= now+300 and now < feed['expiresUnix'] <= feed['createdUnix']+31*86400,'feed_time_invalid')
    for name, names in [('game',GAME_META),('engine',{'ENGINE.json','ENGINE.p7s'})]:
        component = feed[name]
        need(type(component) is dict and set(component) == {'directory','identity','metadata'} and type(component['metadata']) is list,'component_shape_invalid')
        need(isinstance(component['directory'],str) and re.fullmatch(r'releases/[A-Za-z0-9_-]+',component['directory']) is not None
             and isinstance(component['identity'],str),'component_path_invalid')
        found = []
        for item in component['metadata']:
            need(type(item) is dict and set(item) == {'path','size','sha256'} and item['path'] in names
                 and type(item['size']) is int and 0 < item['size'] <= 64*1024**2
                 and isinstance(item['sha256'],str) and re.fullmatch('[0-9A-F]{64}',item['sha256']) is not None,'component_metadata_invalid')
            found.append(item['path'])
        need(len(found) == len(names) and set(found) == names,'component_metadata_set_invalid')
    need(feed['game']['identity'] == game_identity and feed['game']['directory'] == 'releases/game-'+game_identity,'fixed_game_component_required')

def sync_directories(operation):
    for path in {operation['prepared'].parent,operation['current'].parent}:
        sync_directory(path)

class TransactionFailure(RuntimeError):
    def __init__(self,cause,rollback):
        super().__init__('engine_only_origin_transaction_failed')
        self.cause = type(cause).__name__
        self.cause_code = str(cause) if re.fullmatch('[a-z_]{1,100}',str(cause)) else type(cause).__name__
        self.rollback = rollback

def durable_success(health_after, result, receipt_path, writer=None):
    value = dict(result,healthAfter=health_after)
    (writer or write_fresh)(receipt_path,canonical(value)+b'\n')
    return value

def emit_committed_result(result, output=None, errors=None):
    output = sys.stdout if output is None else output
    errors = sys.stderr if errors is None else errors
    payload = json.dumps(result,separators=(',',':'))+'\n'
    try:
        output.write(payload)
        output.flush()
    except (OSError,ValueError):
        # The durable report and feed transaction already completed. A broken
        # stdout is an unknown delivery outcome, never a claim of rollback.
        try:
            errors.write(json.dumps({'status':'output_delivery_unknown','readbackRequired':True,
                                    'durableReport':result['backupDirectory']+'/DEPLOYMENT.json'},separators=(',',':'))+'\n')
            errors.flush()
        except (OSError,ValueError):
            pass
        return 3
    return 0

def exchange_transaction(deploy, operations, before_each, after_all, sync=sync_directories):
    switched = []
    try:
        for operation in operations:
            before_each(operation)
            deploy.exchange_directories(operation['prepared'],operation['current'])
            switched.append(operation)
            sync(operation)
            deploy.verify_serving(operation['newBytes'],operation['newSignature'],operation['newFeed'],operation['scope'])
        result = after_all()
        return result
    except BaseException as cause:
        rollback = []
        # Continue trying every earlier exchange even if one rollback/check fails.
        for operation in reversed(switched):
            result = {'scope':operation['scope'],'restored':False,'servingVerified':False}
            try:
                deploy.exchange_directories(operation['prepared'],operation['current'])
                result['restored'] = True
                sync(operation)
                deploy.verify_serving(operation['oldBytes'],operation['oldSignature'],operation['oldFeed'],operation['scope'])
                result['servingVerified'] = True
            except BaseException as error:
                result['failureType'] = type(error).__name__
            rollback.append(result)
        raise TransactionFailure(cause,rollback) from None

def run(fixed):
    import fcntl, pwd
    need(os.geteuid() == 0,'root_execution_required')
    os.umask(0o077)
    need(fixed['schema'] == 'mir2.fixed-engine-only-origin.v1' and set(fixed['scopes']) == {'legacy','five-class'},'fixed_definition_required')
    deploy = load_reviewed_code(fixed['deployer'],'mir2_fixed_engine_origin_deployer')
    archive = load_reviewed_code(fixed['archiveHelper'],'mir2_fixed_engine_origin_archive')
    need(deploy.ROOT == ROOT and deploy.CADDY == CADDY and deploy.BACKUPS == BACKUPS,'reviewed_code_constants_differ')
    ancestors(BACKUPS,root_owned=True)
    lock_path = BACKUPS / '.publisher.lock'
    plain(lock_path,root_owned=True)
    lock_descriptor = os.open(lock_path,os.O_RDWR | os.O_NOFOLLOW)
    with os.fdopen(lock_descriptor,'rb') as lock:
        need(os.fstat(lock.fileno()).st_uid == 0 and stat.S_ISREG(os.fstat(lock.fileno()).st_mode) and os.fstat(lock.fileno()).st_nlink == 1,'publisher_lock_not_plain')
        fcntl.flock(lock.fileno(),fcntl.LOCK_EX | fcntl.LOCK_NB)
        caddy = checked_bytes(CADDY,fixed['caddy'],1024**2,root_owned=True)
        health_before = deploy.health_idle()
        upload = pathlib.Path(fixed['uploadDirectory'])
        need(upload.parent == pathlib.Path('/home/ubuntu') and re.fullmatch(r'mir2-native-upload-engine2324-[0-9a-f]{32}',upload.name) is not None,'fixed_upload_directory_required')
        upload_uid = pwd.getpwnam('ubuntu').pw_uid
        ancestors(upload)
        plain(upload,directory=True,private_uid=upload_uid)
        incoming = upload/'inputs'
        plain(incoming,directory=True,private_uid=upload_uid)
        need({item.name for item in incoming.iterdir()} == {'legacy','five-class'},'unexpected_input_scope')
        for scope in ['legacy','five-class']:
            directory = incoming/scope
            plain(directory,directory=True,private_uid=upload_uid)
            need({item.name for item in directory.iterdir()} == ENGINE_FILES | {'latest.json','latest.p7s'},'unexpected_upload_input')
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')+'-'+uuid.uuid4().hex[:10]
        stage = private_directory(pathlib.Path('/srv')/('.mir2-engine-only-origin-'+stamp))
        snapshots = private_directory(stage/'snapshots')
        new_feeds, old_feeds, old_data, new_data, metadata_verified = {}, {}, {}, {}, []
        for scope in ['legacy','five-class']:
            snapshot = private_directory(snapshots/scope)
            spec = fixed['scopes'][scope]
            data = {}
            need(set(spec['inputs']) == ENGINE_FILES | {'latest.json','latest.p7s'},'fixed_input_closure_required')
            for name,item in spec['inputs'].items():
                value = checked_bytes(incoming/scope/name,item,128*1024**2 if name == 'Mir2Updater.exe' else 32768,private_uid=upload_uid)
                write_fresh(snapshot/name,value)
                data[name] = value
            release_root = ROOT/'five-class' if scope == 'five-class' else ROOT
            ancestors(release_root,root_owned=True)
            current = release_root/'feed-current'
            ancestors(current,root_owned=True)
            need({item.name for item in current.iterdir()} == {'latest.json','latest.p7s'},'old_feed_directory_closure_differs')
            previous = {name:checked_bytes(current/name,spec['oldInputs'][name],32768,root_owned=True) for name in ['latest.json','latest.p7s']}
            old_feed = archive.json_bytes(previous['latest.json'],'old feed',32768)
            new_feed = archive.json_bytes(data['latest.json'],'new feed',32768)
            feed_shape(old_feed,spec['oldSequence'],spec['gameIdentity'])
            feed_shape(new_feed,spec['newSequence'],spec['gameIdentity'])
            need(canonical(new_feed['game']) == canonical(old_feed['game']),'new_feed_changed_game_component')
            need(canonical(new_feed) == canonical(spec['newFeed']) and canonical(old_feed) == canonical(spec['oldFeed']),'fixed_feed_structure_differs')
            for item in old_feed['game']['metadata']:
                checked_bytes(release_root/old_feed['game']['directory']/item['path'],item,64*1024**2,root_owned=True)
                metadata_verified.append({'scope':scope,'path':old_feed['game']['directory']+'/'+item['path'],'size':item['size'],'sha256':item['sha256']})
            new_feeds[scope],old_feeds[scope],new_data[scope],old_data[scope] = new_feed,old_feed,data,previous
        backup = private_directory(BACKUPS/('engine2324-'+stamp))
        write_fresh(backup/'Caddyfile.before',caddy)
        write_fresh(backup/'FIXED-DEFINITION.json',canonical(fixed)+b'\n')
        for scope in ['legacy','five-class']:
            directory = private_directory(backup/scope)
            for name,value in old_data[scope].items():
                write_fresh(directory/name,value)
        operations,promotions = [],{}
        for scope in ['five-class','legacy']:
            scope_stage = private_directory(stage/scope)
            release_parent = private_directory(scope_stage/'releases')
            feed = new_feeds[scope]
            component = feed['engine']['directory']
            need(re.fullmatch(r'releases/updater-[0-9A-F]{16}-s'+str(fixed['scopes'][scope]['newSequence']),component) is not None,'fixed_engine_directory_required')
            engine_stage = private_directory(scope_stage/component)
            for name in ENGINE_FILES:
                write_fresh(engine_stage/name,new_data[scope][name])
            files,_,_ = archive.scan({component:str(engine_stage)})
            identity = archive.validate_engine(files[component])
            descriptor,_ = archive._metadata(files[component],'ENGINE.json',limit=32768)
            need(descriptor['sourceRevision'].lower() == fixed['engineSourceRevision'] and identity['identity'] == feed['engine']['identity'],'new_engine_identity_mismatch')
            for item in feed['engine']['metadata']:
                entry = files[component][item['path']]
                need((entry.size,entry.sha256) == (item['size'],item['sha256']),'engine_feed_metadata_mismatch')
            need(descriptor['exeSha256'][:16] in component and (descriptor['exeSize'],descriptor['exeSha256']) == (fixed['engineExe']['size'],fixed['engineExe']['sha256']),'fixed_engine_executable_mismatch')
            release_root = ROOT/'five-class' if scope == 'five-class' else ROOT
            ancestors(release_root/'releases',root_owned=True)
            promotions[scope] = deploy.promote_immutable(scope_stage,component,files[component],archive,release_root)
            sync_directory(release_root/component)
            sync_directory(release_root/'releases')
            prepared = private_directory(scope_stage/'feed-prepared')
            for name in ['latest.json','latest.p7s']:
                write_fresh(prepared/name,new_data[scope][name],0o644)
            os.chmod(prepared,0o755)
            operations.append({'scope':scope,'prepared':prepared,'current':release_root/'feed-current',
                               'newBytes':new_data[scope]['latest.json'],'newSignature':new_data[scope]['latest.p7s'],'newFeed':feed,
                               'oldBytes':old_data[scope]['latest.json'],'oldSignature':old_data[scope]['latest.p7s'],'oldFeed':old_feeds[scope]})
        def before_each(operation):
            need(checked_bytes(CADDY,fixed['caddy'],1024**2,root_owned=True) == caddy,'caddy_changed_during_staging')
            for name in ['latest.json','latest.p7s']:
                checked_bytes(operation['current']/name,fixed['scopes'][operation['scope']]['oldInputs'][name],32768,root_owned=True)
        success = {'schema':'mir2.engine-only-origin-deployment.v1','passed':True,'checkedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
                   'engineSourceRevision':fixed['engineSourceRevision'],'engineExe':fixed['engineExe'],'sequences':{'legacy':23,'five-class':24},
                   'planSha256':fixed['planSha256'],'caddySha256':fixed['caddy']['sha256'],'caddyChanged':False,
                   'signatureValidation':'workstation-CMS-admitted-before-SHA256-pinned-upload','immutableEngines':promotions,
                   'gameMetadataRehashed':metadata_verified,'fullGamePayloadRehashed':False,'healthBefore':health_before,
                   'backendMutated':False,'databaseWritten':False,'dockerStarted':False,'backupDirectory':str(backup),'retainedStageDirectory':str(stage)}
        def after_all():
            need(checked_bytes(CADDY,fixed['caddy'],1024**2,root_owned=True) == caddy,'caddy_changed_after_feed_exchange')
            # Report write, file fsync and parent-directory fsync are part of
            # the rollback-covered commit, after serving/health checks.
            return durable_success(deploy.health_idle(),success,backup/'DEPLOYMENT.json')
        deploy.health_idle()
        try:
            result = exchange_transaction(deploy,operations,before_each,after_all)
        except TransactionFailure as error:
            failure = {'schema':'mir2.engine-only-origin-deployment.v1','passed':False,'failure':'engine_only_origin_transaction_failed',
                       'causeType':error.cause,'causeCode':error.cause_code,'rollback':error.rollback,'backupDirectory':str(backup),'retainedStageDirectory':str(stage)}
            # A failed report fsync can leave complete JSON on disk. Preserve
            # it with an explicitly uncommitted name, never as final evidence.
            report = backup/'DEPLOYMENT.json'
            if os.path.lexists(report):
                try:
                    plain(report,root_owned=True)
                    quarantined = backup/'DEPLOYMENT-UNCOMMITTED.json'
                    need(not os.path.lexists(quarantined),'uncommitted_report_path_already_exists')
                    os.rename(report,quarantined)
                    sync_directory(backup)
                    failure['uncommittedReportQuarantined'] = True
                except BaseException as quarantine_error:
                    failure['uncommittedReportQuarantined'] = False
                    failure['quarantineFailureType'] = type(quarantine_error).__name__
            try:
                write_fresh(backup/'FAILURE.json',canonical(failure)+b'\n')
                failure['failureRecordDurable'] = True
            except BaseException as record_error:
                failure['failureRecordDurable'] = False
                failure['failureRecordFailureType'] = type(record_error).__name__
            print(json.dumps(failure,separators=(',',':')),flush=True)
            return 1
        return emit_committed_result(result)

def entrypoint(fixed):
    try:
        return run(fixed)
    except BaseException as error:
        code = str(error) if re.fullmatch('[a-z_]{1,100}',str(error)) else type(error).__name__
        print(json.dumps({'passed':False,'failure':'engine_only_origin_admission_failed','causeType':type(error).__name__,'causeCode':code},separators=(',',':')))
        return 1

FIXED = json.loads('{"archiveHelper":{"path":"/var/lib/mir2-client-publisher/E1DDE2A0B4E59A2B1F02EF9A69F8E4B652B7104D3DB00B7D565E6C1437C860EB.py","sha256":"E1DDE2A0B4E59A2B1F02EF9A69F8E4B652B7104D3DB00B7D565E6C1437C860EB","size":25056},"caddy":{"sha256":"F27E305C8D741DF564DF906D01FB7B98B31260208394AF01D585073D89EB1CB1","size":6650},"deployer":{"path":"/var/lib/mir2-client-publisher/395791C74846505A344ADF31F0DA3F1028F897C02DE9646FD15A6403D13269B2.py","sha256":"395791C74846505A344ADF31F0DA3F1028F897C02DE9646FD15A6403D13269B2","size":33734},"engineExe":{"sha256":"C8241025681123CE3E9B24F3E78AAC17577B24833395F7BB2644B8905AAFC5DE","size":1959936},"engineSourceRevision":"0164ae44637cbac06f967a5732ca62a74221063f","planSha256":{"five-class":{"new":"6b3fe5c87b1dc9d6f3d5af8a9516a9488750ccc3f21530d3e0ffc87572b7a409","newVerification":"4f993398b63ad539584e75caddfff4028fe11a65de8d75232dc0a10defea853b","old":"5473cfe052e8715ef3f9aa9521239fd94a0d1ad343dd119c1d47a35795869311","oldVerification":"cde62acd12be18ed5985a050e85d54565c3d4ddadb5c6962475b0fc2d998a522"},"legacy":{"new":"365273255721abe49ba793b6853fb671ce36e3ea78808330fcca95f9893f79a0","newVerification":"fc3e9cd39c04e4efdd577608efc083959852fbf932498e45fed55813ebc73467","old":"3282c9c65c9c77292f54db8ad7659f2bd2fef6b51e3b867bedfb66aac0b674ae","oldVerification":"63d565f4ea92f49b3876911929d28347fbf2c0dd89857069aaf5289b63f02f72"}},"schema":"mir2.fixed-engine-only-origin.v1","scopes":{"five-class":{"gameIdentity":"WN-CANDIDATE-20261029-wallet-28","inputs":{"ENGINE.json":{"sha256":"46C46A88E908D09E224E41DDC9D98C3CC62B297178CC05468B4E37D6B96A03C4","size":421},"ENGINE.p7s":{"sha256":"A318EB498484D8A0DE9EE6BBB847FF7ACFA0965BBE4531469118508308F2DCBE","size":1614},"Mir2Updater.exe":{"sha256":"C8241025681123CE3E9B24F3E78AAC17577B24833395F7BB2644B8905AAFC5DE","size":1959936},"latest.json":{"sha256":"0C7C9626271C98D478A48CE79DCA92F31436B84AD20B6C2DB4462D65B4DE8A91","size":1157},"latest.p7s":{"sha256":"E691787D7AD8E69F7936573527C7A8650D4B5701AA46D6F9AE2F66CB098E90B6","size":1614}},"newFeed":{"channel":"invited","content":"mir2.windows.package-manifest.v4","createdUnix":1793218757,"engine":{"directory":"releases/updater-C8241025681123CE-s24","identity":"1","metadata":[{"path":"ENGINE.json","sha256":"46C46A88E908D09E224E41DDC9D98C3CC62B297178CC05468B4E37D6B96A03C4","size":421},{"path":"ENGINE.p7s","sha256":"A318EB498484D8A0DE9EE6BBB847FF7ACFA0965BBE4531469118508308F2DCBE","size":1614}]},"expiresUnix":1795810757,"game":{"directory":"releases/game-WN-CANDIDATE-20261029-wallet-28","identity":"WN-CANDIDATE-20261029-wallet-28","metadata":[{"path":"PACKAGE-MANIFEST.json","sha256":"BCA71514A99E158EB583F48D3E81095219C81C0C122CB1AE498B081460F822C0","size":49712342},{"path":"VERSION.json","sha256":"C54DC2C9D8301E1793B8316AFE6CAB554DF19FC62468D6005F41C7897DBC802F","size":1220},{"path":"RELEASE-STATEMENT.json","sha256":"FB23EE4AA828C74C4F350ADF392D08A3619890003898993D672DF7665477111E","size":706},{"path":"RELEASE-STATEMENT.p7s","sha256":"715446E2E78EFFF3EA8FDEEF8D99E5E2770A7F99A03352DA6B873A5529CC6B5B","size":1614}]},"minBootstrap":1,"platform":"windows-x64","protocol":"crystal-mir2-v1","schema":"mir2.windows.update-feed.v1","sequence":24},"newSequence":24,"oldFeed":{"channel":"invited","content":"mir2.windows.package-manifest.v4","createdUnix":1793212597,"engine":{"directory":"releases/updater-F2F1AB36F3ECA608-s22","identity":"1","metadata":[{"path":"ENGINE.json","sha256":"2F2D6132FE3BFE23A57E9D05C4B3D98FD68F9247DDBBF3043812DD58F24A27D8","size":421},{"path":"ENGINE.p7s","sha256":"5F0A6D296356304D04E2C73233512624699B6C6AA646D8D099DCE7B176E66C46","size":1614}]},"expiresUnix":1795804597,"game":{"directory":"releases/game-WN-CANDIDATE-20261029-wallet-28","identity":"WN-CANDIDATE-20261029-wallet-28","metadata":[{"path":"PACKAGE-MANIFEST.json","sha256":"BCA71514A99E158EB583F48D3E81095219C81C0C122CB1AE498B081460F822C0","size":49712342},{"path":"VERSION.json","sha256":"C54DC2C9D8301E1793B8316AFE6CAB554DF19FC62468D6005F41C7897DBC802F","size":1220},{"path":"RELEASE-STATEMENT.json","sha256":"FB23EE4AA828C74C4F350ADF392D08A3619890003898993D672DF7665477111E","size":706},{"path":"RELEASE-STATEMENT.p7s","sha256":"715446E2E78EFFF3EA8FDEEF8D99E5E2770A7F99A03352DA6B873A5529CC6B5B","size":1614}]},"minBootstrap":1,"platform":"windows-x64","protocol":"crystal-mir2-v1","schema":"mir2.windows.update-feed.v1","sequence":22},"oldInputs":{"latest.json":{"sha256":"B53E97C77D63930388A60D927D283FD89E9FE754C431A5FBA66EF35A9B2CD5AD","size":1157},"latest.p7s":{"sha256":"4C234AB7CE990A761499A198EC9A1EDAD708C9372FBAFDA16CF9DAB483E90C2D","size":1614}},"oldSequence":22},"legacy":{"gameIdentity":"WN-CANDIDATE-20261011-invited-25","inputs":{"ENGINE.json":{"sha256":"1BFA1E4CF5860E9E28A0CDE247A70CDB79CE8C45A9064E10CDF81A320A830BB6","size":421},"ENGINE.p7s":{"sha256":"DDAA26DA20192E165755B5AD47D9D685A44963F910FCDAB532B698F147D06065","size":1614},"Mir2Updater.exe":{"sha256":"C8241025681123CE3E9B24F3E78AAC17577B24833395F7BB2644B8905AAFC5DE","size":1959936},"latest.json":{"sha256":"CFC78A45E30C8079857CFFB2BF6C02C8266A1C9C85FEF4022368B43E8D11317C","size":1159},"latest.p7s":{"sha256":"A59383D661F1B16CE18A62F47EC6C78C3A3CCABF462A5FC2637DFE036211712A","size":1614}},"newFeed":{"channel":"invited","content":"mir2.windows.package-manifest.v4","createdUnix":1793218753,"engine":{"directory":"releases/updater-C8241025681123CE-s23","identity":"1","metadata":[{"path":"ENGINE.json","sha256":"1BFA1E4CF5860E9E28A0CDE247A70CDB79CE8C45A9064E10CDF81A320A830BB6","size":421},{"path":"ENGINE.p7s","sha256":"DDAA26DA20192E165755B5AD47D9D685A44963F910FCDAB532B698F147D06065","size":1614}]},"expiresUnix":1795810753,"game":{"directory":"releases/game-WN-CANDIDATE-20261011-invited-25","identity":"WN-CANDIDATE-20261011-invited-25","metadata":[{"path":"PACKAGE-MANIFEST.json","sha256":"8F50807C9171B28D77424A113C2A55A8AB5DCDD507E3051222C49863E2EFC748","size":31538315},{"path":"VERSION.json","sha256":"481DA3F3EE5AFAB88A9B20F2DEB5A70E359B859C53671D9099B6213E5748FBAB","size":1221},{"path":"RELEASE-STATEMENT.json","sha256":"60405C35155AEDFF69FA87B42212780BF5A02D9E668FCCA567F3A45DCA7ACF10","size":707},{"path":"RELEASE-STATEMENT.p7s","sha256":"88C2AA59547FC111FB94338E542327130BE7F4429DB87915B3CA39B018AD171F","size":1614}]},"minBootstrap":1,"platform":"windows-x64","protocol":"crystal-mir2-v1","schema":"mir2.windows.update-feed.v1","sequence":23},"newSequence":23,"oldFeed":{"channel":"invited","content":"mir2.windows.package-manifest.v4","createdUnix":1793206287,"engine":{"directory":"releases/updater-F2F1AB36F3ECA608-s21","identity":"1","metadata":[{"path":"ENGINE.json","sha256":"E2FB71018A88ABC95D879890210296C70AE6230C6EC45072FDD7D20585CB8756","size":421},{"path":"ENGINE.p7s","sha256":"E4E35E8CF744A38ADA77203A1E56BDEEB1DD402E356AE9EAAF2621C617B92D58","size":1614}]},"expiresUnix":1795798287,"game":{"directory":"releases/game-WN-CANDIDATE-20261011-invited-25","identity":"WN-CANDIDATE-20261011-invited-25","metadata":[{"path":"PACKAGE-MANIFEST.json","sha256":"8F50807C9171B28D77424A113C2A55A8AB5DCDD507E3051222C49863E2EFC748","size":31538315},{"path":"VERSION.json","sha256":"481DA3F3EE5AFAB88A9B20F2DEB5A70E359B859C53671D9099B6213E5748FBAB","size":1221},{"path":"RELEASE-STATEMENT.json","sha256":"60405C35155AEDFF69FA87B42212780BF5A02D9E668FCCA567F3A45DCA7ACF10","size":707},{"path":"RELEASE-STATEMENT.p7s","sha256":"88C2AA59547FC111FB94338E542327130BE7F4429DB87915B3CA39B018AD171F","size":1614}]},"minBootstrap":1,"platform":"windows-x64","protocol":"crystal-mir2-v1","schema":"mir2.windows.update-feed.v1","sequence":21},"oldInputs":{"latest.json":{"sha256":"4EAD3D8FDBC12838A03141F229585A997374346FDE566333D44860EAE7B78650","size":1159},"latest.p7s":{"sha256":"13D6D1DDD986B208ADE876AC32CAF7887AF138FBF7C71B9B554232A80FFF3A7A","size":1614}},"oldSequence":21}},"uploadDirectory":"/home/ubuntu/mir2-native-upload-engine2324-ac321a8ac9cb4ecbac8027b1fb773527"}\n')
if __name__ == '__main__':
    raise SystemExit(entrypoint(FIXED))

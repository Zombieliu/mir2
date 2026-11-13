"""Explicit, bounded, read-only TEST map collection. Import/--help do no SSH.

The old inventory is an expectation, never the fresh sample. No credentials,
raw environment, command lines, unknown process names, or unparsed output persist.
The coordinator must explicitly invoke --collect in a PTY. No service actions.
"""
import argparse
import base64
import getpass
import hashlib
import hmac
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import socket
import sys
import threading
import time
import warnings


HOST = "165.154.65.136"
PIN = "SHA256:6M4ouOk6QvFj1qIEmjj9/LOa8/enGiqZ/n+m7JHo7q8"
RUNTIME = Path("C:/Users/Administrator/.codex/tmp/mir2-deploy-ssh-runtime")
TRANSPORT_BASELINE_SHA256 = "86a7c30f4bd514987db063a3731e0e6f980799b83944f2fba7e3f9d1dc745098"
BASELINE = "82efff2c8c0eef72a950501d84acad3c6c3de958"
BASELINE_BINARY = "6ed4ee817b2577858860a9bf53cea12845d5ab2cb0d855eb5ffa678ba9ccc01a"
CANDIDATE = "5a356ae0d5b79b846675f623bd5d2dc21c09a10f"
SCHEMA = "mir2.scene-live-map-proof.v6"  # Frozen V7 verifier's live-input DTO.
MARKER = b"MIR2_SCENE_READONLY_LIVE_MAPS_V1 "
REMOTE_SECONDS, LOCAL_SECONDS, OUTPUT_BOUND = 20, 60, 262144
FILE_COUNT, FILE_BYTES = 1620, 42111309
MAP_TREE = "mir2-web3/apps/web/lib/generated/crystal-map-pack"
ROLE_HINTS = [
    {"role": "old-gateway", "pid": 3855184}, {"role": "old-zone", "pid": 3855181},
    {"role": "caddy", "pid": 2107428},
    *[{"role": "validator-" + str(i), "pid": p} for i, p in enumerate([2129892, 2130026, 2130154, 2130288])],
    {"role": "vpn-0", "pid": 1520883},
]


REMOTE = r'''
import base64,hashlib,json,os,pathlib,re,signal,stat,subprocess,sys,time,urllib.request
P=pathlib.Path
SCHEMA='mir2.scene-live-map-proof.v6'
MARKER='MIR2_SCENE_READONLY_LIVE_MAPS_V1 '
DEADLINE=None
UNIT='mir2-playtest.service'
class AuditError(RuntimeError):pass
def need(value,code):
 if not value:raise AuditError(code)
def left(cap=20):
 remaining=DEADLINE-time.monotonic()
 if remaining<=0:raise AuditError('remote_query_budget')
 return min(cap,remaining)
def alarm(*unused):raise AuditError('remote_query_budget')
def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':')).encode()
def digest(data):return hashlib.sha256(data).hexdigest()
def read(path,limit=16384):
 left()
 with P(path).open('rb') as stream:data=stream.read(limit+1)
 need(len(data)<=limit,'bounded_read_exceeded')
 return data
def proc_state(pid):
 raw=read(P('/proc',str(pid),'stat'))
 fields=raw.rsplit(b')',1)[1].split()
 need(fields[0] not in [b'Z',b'X'] and int(fields[19])>0,'process_not_live')
 return int(fields[19])
def unit():
 props=['MainPID','InvocationID','ActiveState','SubState','ControlGroup','CPUQuotaPerSecUSec','CPUQuotaPeriodUSec','MemoryMax','MemoryHigh','TasksMax']
 result=subprocess.run(['systemctl','show',UNIT,'--no-pager']+['--property='+p for p in props],stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,check=True,timeout=left(3))
 need(len(result.stdout)<=16384,'unit_output_bound')
 rows=result.stdout.splitlines();need(all('=' in row for row in rows),'unit_format')
 fields=dict(row.split('=',1) for row in rows);need(len(rows)==len(fields),'unit_duplicate_property')
 pid=int(fields.get('MainPID','0'));inv=fields.get('InvocationID','')
 need(pid>1 and re.fullmatch('[0-9a-f]{32}',inv),'unit_identity_invalid')
 return {'pid':pid,'invocationId':inv,'activeState':fields.get('ActiveState'),'subState':fields.get('SubState'),
  'processStartTicks':proc_state(pid),'controlGroup':fields.get('ControlGroup'),
  'systemdLimits':{p:fields.get(p) if fields.get(p) in {'infinity','[not set]'} or re.fullmatch(r'[0-9]+(?:us|ms|s)?',fields.get(p,'')) else None for p in props[5:]}}
def stable(st):return (st.st_dev,st.st_ino,st.st_size,st.st_mtime_ns,st.st_ctime_ns,st.st_mode,st.st_nlink)
def hash_fd(fd,maximum,expected=None):
 before=os.fstat(fd);need(stat.S_ISREG(before.st_mode) and 0<=before.st_size<=maximum,'regular_file_size_required')
 if expected is not None:need(stable(before)==expected,'opened_inode_changed')
 h=hashlib.sha256();size=0;prefix=b''
 while True:
  left();chunk=os.read(fd,1048576)
  if not chunk:break
  size+=len(chunk);need(size<=maximum,'stream_byte_bound')
  if not prefix:prefix=chunk[:128]
  h.update(chunk)
 need(size==before.st_size and stable(os.fstat(fd))==stable(before),'file_changed_during_hash')
 return {'bytes':size,'sha256':h.hexdigest()},prefix
def exe_fingerprint(pid):
 path=P('/proc',str(pid),'exe');name=os.readlink(path)
 need(not name.endswith(' (deleted)') and name.startswith('/'),'deleted_or_relative_executable')
 fd=os.open(path,os.O_RDONLY|os.O_CLOEXEC)
 try:row,unused=hash_fd(fd,268435456)
 finally:os.close(fd)
 return name,row
def clean_absolute(path):
 path=P(path);need(path.is_absolute() and '..' not in path.parts,'absolute_nontraversing_path_required')
 current=P('/')
 for part in path.parts[1:]:
  current=current/part;st=current.lstat()
  need(stat.S_ISDIR(st.st_mode) and not stat.S_ISLNK(st.st_mode),'directory_symlink_or_type_rejected')
 need(path.resolve(strict=True)==path,'physical_path_required')
 return path
def cwd_allowed(value):
 path=pathlib.PurePosixPath(value)
 return path.is_absolute() and path.as_posix()==value and '..' not in path.parts and (value=='/var/lib/mir2-playtest/state' or path.is_relative_to('/opt/mir2-playtest'))
def map_location(pid):
 proc=P('/proc',str(pid));raw=read(proc/'environ',262144)
 # Do not decode or project any other environment key.
 values=[row.split(b'=',1)[1] for row in raw.split(b'\0') if row.startswith(b'MIR2_CRYSTAL_MAP_PACK=')]
 del raw
 need(len(values)==1 and values[0],'explicit_unique_map_pack_required')
 value=values[0].decode('utf-8','strict');del values
 need(len(value)<=4096 and '\n' not in value and '\r' not in value,'map_path_format')
 cwd_value=os.readlink(proc/'cwd');need(cwd_allowed(cwd_value),'cwd_outside_readonly_scope');cwd=P(cwd_value)
 clean_absolute(cwd)
 root=P(value);relative=not root.is_absolute();root=cwd/root if relative else root
 root=clean_absolute(root)
 need(root.is_relative_to(P('/opt/mir2-playtest')) and root!=P('/opt/mir2-playtest'),'map_root_outside_readonly_scope')
 return {'workingDirectory':str(cwd),'mapRoot':str(root),'mapRootSource':'MIR2_CRYSTAL_MAP_PACK','mapValueWasRelative':relative}
class NoRedirect(urllib.request.HTTPRedirectHandler):
 def redirect_request(self,*unused,**kwargs):raise AuditError('health_redirect_refused')
def health():
 opener=urllib.request.build_opener(urllib.request.ProxyHandler({}),NoRedirect())
 with opener.open('http://127.0.0.1:7210/health',timeout=left(3)) as response:
  need(response.status==200,'health_http_status');raw=response.read(1048577)
 need(len(raw)<=1048576,'health_read_bound');value=json.loads(raw)
 return {'ok':value.get('ok'),'revision':value.get('revision')}
def snapshot(expected):
 target=unit();need(target==expected['target'],'expected_target_fingerprint_mismatch')
 pid=target['pid'];name,binary=exe_fingerprint(pid)
 need(name==expected['executablePath'] and binary['sha256']==expected['binarySha256'] and binary['bytes']==expected['binaryBytes'],'expected_executable_mismatch')
 location=map_location(pid);h=health()
 need(h=={'ok':True,'revision':expected['sourceRevision']},'expected_healthy_source_mismatch')
 need(unit()==target,'target_changed_within_snapshot')
 return {'target':target,'executable':{'path':name,**binary},'sourceRevision':h['revision'],**location}
def relative_name(value):
 path=pathlib.PurePosixPath(value)
 need(isinstance(value,str) and value==path.as_posix() and not path.is_absolute() and '..' not in path.parts and re.fullmatch(r'[A-Za-z0-9_./ -]+',value) and value.endswith('.map.gz'),'map_relative_path_invalid')
 return value
def tree(root):
 rows=[];directories=[];total=0
 for directory,dirs,names in os.walk(root,followlinks=False):
  left();base=P(directory);directories.append((base.relative_to(root).as_posix(),stable(base.lstat())))
  need(len(directories)<=256,'map_directory_count_bound')
  for name in dirs:
   need(stat.S_ISDIR((base/name).lstat().st_mode),'map_directory_symlink_rejected')
  for name in names:
   path=base/name;key=relative_name(path.relative_to(root).as_posix());st=path.lstat()
   need(stat.S_ISREG(st.st_mode) and st.st_nlink==1 and 0<st.st_size<=16777216,'map_type_link_or_size_rejected')
   total+=st.st_size;need(len(rows)<16384 and total<=629145600,'map_tree_count_or_byte_bound')
   rows.append((key,stable(st)))
 return sorted(rows),sorted(directories)
def root_fd(root):return os.open(root,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC)
def member_fd(root_descriptor,name):
 parts=pathlib.PurePosixPath(relative_name(name)).parts;parent=os.dup(root_descriptor)
 try:
  for part in parts[:-1]:
   following=os.open(part,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC,dir_fd=parent);os.close(parent);parent=following
  return os.open(parts[-1],os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC,dir_fd=parent)
 finally:os.close(parent)
def map_files(root,expected):
 initial,dirs=tree(root);need(len(initial)==expected['fileCount'],'map_count_mismatch')
 fd=root_fd(root);files=[]
 try:
  need(stable(os.fstat(fd))==stable(root.lstat()),'root_inode_mismatch')
  for name,prior in initial:
   opened=member_fd(fd,name)
   try:row,prefix=hash_fd(opened,16777216,prior)
   finally:os.close(opened)
   need(not prefix.startswith(b'version https://git-lfs.github.com/spec/v1'),'lfs_pointer_refused')
   files.append({'path':name,**row})
  need(tree(root)==(initial,dirs) and stable(os.fstat(fd))==stable(root.lstat()),'map_tree_changed_during_hash')
 finally:os.close(fd)
 need(sum(row['bytes'] for row in files)==expected['totalFileBytes'] and digest(canonical(files))==expected['normalizedFilesSha256'],'map_manifest_rows_mismatch')
 return files
def normalized_manifest(value):
 need(isinstance(value,dict) and value.get('sourceRevision') in ['82efff2c8c0eef72a950501d84acad3c6c3de958','5a356ae0d5b79b846675f623bd5d2dc21c09a10f'],'live_manifest_source_invalid')
 rows=value.get('files');need(isinstance(rows,list) and len(rows)<=16384,'live_manifest_rows_invalid');out=[]
 for row in rows:
  need(isinstance(row,dict) and set(row)=={'path','bytes','sha256'} and isinstance(row.get('path'),str) and row['path'].startswith('crystal-map-pack/'),'manifest_member_invalid')
  name=relative_name(row['path'][len('crystal-map-pack/'):]);need(type(row['bytes']) is int and 0<row['bytes']<=16777216 and re.fullmatch('[0-9a-f]{64}',row.get('sha256','')),'manifest_member_invalid')
  out.append({**row,'path':name})
 need(out==sorted(out,key=lambda r:r['path']) and len({r['path'].casefold() for r in out})==len(out) and value.get('fileCount')==len(out) and value.get('totalFileBytes')==sum(r['bytes'] for r in out),'manifest_totals_or_order_invalid')
 return out
def live_manifest(expected):
 path=P(expected['executablePath']).parent/'crystal-map-pack-manifest.json'
 if not path.exists() and not path.is_symlink():return {'present':False,'path':str(path),'reason':'not_installed_at_release_root'}
 st=path.lstat();need(stat.S_ISREG(st.st_mode) and st.st_nlink==1 and 0<st.st_size<=1048576,'live_manifest_type_or_size')
 clean_absolute(path.parent);fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 try:
  row,unused=hash_fd(fd,1048576,stable(st));os.lseek(fd,0,os.SEEK_SET);raw=os.read(fd,1048577)
  need(len(raw)==row['bytes'] and digest(raw)==row['sha256'] and stable(os.fstat(fd))==stable(st),'live_manifest_changed')
 finally:os.close(fd)
 files=normalized_manifest(json.loads(raw));need(digest(canonical(files))==expected['normalizedFilesSha256'],'live_manifest_maps_mismatch')
 need(stable(path.lstat())==stable(st),'live_manifest_inode_changed')
 return {'present':True,'path':str(path),**row,'normalizedFilesSha256':digest(canonical(files))}
VPN_EXES={'xray','xray-linux-amd64','v2ray','v2ray-linux-amd64','sing-box','hysteria','hysteria2','openvpn','tailscaled','wireguard-go','zerotier-one','nebula','clash','clash-meta','mihomo','gost','tuic-server','ss-server'}
def family(role,comm,executable):
 name=P(executable).name
 allow={'old-gateway':{'mir2-gateway'},'old-zone':{'zone_host'},'caddy':{'caddy'}}
 allowed=VPN_EXES if role.startswith('vpn-') else ({'gate14_validator','gate14-validator','gate14_validator-linux-x64','gate14-validator-linux-x64'} if role.startswith('validator-') else allow.get(role,set()))
 if name not in allowed or comm!=name[:15]:return None
 return 'known-vpn' if role.startswith('vpn-') else ('gate14-validator' if role.startswith('validator-') else role)
def protected(hints):
 rows=[];fingerprints=[]
 for hint in hints:
  left();role,pid=hint['role'],hint['pid']
  try:
   start=proc_state(pid);proc=P('/proc',str(pid));comm=read(proc/'comm',128);name=os.readlink(proc/'exe')
   matched=family(role,comm.decode('ascii','strict').strip(),name)
   if matched is None:
    rows.append({'role':role,'pid':pid,'status':'unknown_family'});continue
   actual_name,binary=exe_fingerprint(pid);cgroup=read(proc/'cgroup',16384)
   need(proc_state(pid)==start and actual_name==name and read(proc/'comm',128)==comm,'protected_identity_changed')
   fingerprint={'role':role,'pid':pid,'startTicks':start,'executableSha256':binary['sha256'],'commSha256':digest(comm),'cgroupSha256':digest(cgroup)}
   rows.append({'role':role,'pid':pid,'status':'identified','family':matched});fingerprints.append(fingerprint)
  except (OSError,UnicodeError):rows.append({'role':role,'pid':pid,'status':'unavailable'})
 return rows,sorted(fingerprints,key=lambda r:r['role'])
def run(config):
 global DEADLINE
 need(os.geteuid()==0,'root_read_permissions_required');DEADLINE=time.monotonic()+20
 signal.signal(signal.SIGALRM,alarm);signal.alarm(20)
 started=round(time.time()*1000);expected=config['expected'];before=snapshot(expected)
 role_before,fp_before=protected(config['roleHints']);manifest_before=live_manifest(expected)
 files=map_files(P(before['mapRoot']),expected)
 manifest_after=live_manifest(expected);role_after,fp_after=protected(config['roleHints']);after=snapshot(expected);left()
 need(before==after,'target_source_or_map_root_changed');need(manifest_before==manifest_after,'live_manifest_changed_during_read')
 need(role_before==role_after and fp_before==fp_after,'protected_processes_changed_during_read')
 identified={r['role'] for r in role_after if r['status']=='identified'}
 legacy_complete={'old-gateway','old-zone'}<=identified
 declared={r['role'] for r in config['roleHints'] if r['role'] not in ['old-gateway','old-zone']}
 roles_complete=declared<=identified and {'caddy','validator-0','validator-1','validator-2','validator-3'}<=identified and any(r.startswith('vpn-') for r in identified)
 def ident(frame):return {'pid':frame['target']['pid'],'invocationId':frame['target']['invocationId'],'startTicks':frame['target']['processStartTicks']}
 result={'schema':SCHEMA,'complete':legacy_complete and roles_complete,'mapsComplete':True,'readOnly':True,
  'atUnixMs':round(time.time()*1000),'startedAtUnixMs':started,'sourceRevision':before['sourceRevision'],
  'binarySha256':before['executable']['sha256'],'identityBefore':ident(before),'identityAfter':ident(after),
  'targetBefore':before,'targetAfter':after,'fileCount':len(files),'totalFileBytes':sum(r['bytes'] for r in files),'files':files,
  'normalizedFilesSha256':digest(canonical(files)),'liveManifest':manifest_after,'expectedManifestSha256':config['mapManifestSha256'],
  'expectedInventorySha256':config['inventorySha256'],'legacyRolesComplete':legacy_complete,'protectedRolesComplete':roles_complete,
  'protectedDiscovery':role_after,'protectedFingerprintsBefore':fp_before,'protectedFingerprintsAfter':fp_after,
  'rolesJSON':{'roles':[{'role':r['role'],'pid':r['pid']} for r in role_after if r['status']=='identified' and r['role'] not in ['old-gateway','old-zone']] if roles_complete else []},
  'readonlyFileChecks':{'allBytesHashed':True,'regularFilesOnly':True,'noSymlinks':True,'noHardlinkedMapFiles':True,'inodeMetadataBeforeAfter':True,'treeEnumeratedBeforeAfter':True},
  'serviceOperations':[],'configurationChanges':[],'remoteQueryMaxSeconds':20}
 signal.alarm(0);return result
if __name__=='__main__':
 try:
  config=json.loads(base64.b64decode(sys.argv[1],validate=True));result=run(config)
 except Exception as error:
  result={'schema':SCHEMA,'complete':False,'errorClass':type(error).__name__}
  if isinstance(error,AuditError):result['errorCode']=str(error)
 text=json.dumps(result,separators=(',',':'));need(len(text.encode())<=262144,'projected_output_bound')
 print(MARKER+text,flush=True);raise SystemExit(0 if result.get('complete') is True else 1)
'''


def require(value, code):
    if not value:
        raise ValueError(code)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def sha_string(value):
    return isinstance(value, str) and re.fullmatch("[0-9a-f]{64}", value) is not None


def local_file(path, maximum):
    path = Path(path)
    require(path.is_absolute() and path.drive.lower() == "c:", "absolute_C_path_required")
    for part in [path, *path.parents]:
        if part.exists():
            st = part.lstat()
            require(not part.is_symlink() and not getattr(st, "st_file_attributes", 0) & 0x400, "local_reparse_rejected")
    require(path.is_file() and path.stat().st_nlink == 1 and path.stat().st_size <= maximum, "bounded_regular_local_file_required")
    return path


def json_input(path, expected_sha, maximum):
    require(sha_string(expected_sha), "explicit_input_sha256_required")
    path = local_file(path, maximum)
    data = path.read_bytes()
    require(sha(data) == expected_sha, "local_input_sha_mismatch")
    def unique(rows):
        out = {}
        for key, value in rows:
            require(key not in out, "duplicate_json_key")
            out[key] = value
        return out
    return json.loads(data, object_pairs_hook=unique)


def expectation(inventory, manifest):
    require(inventory.get("schema") == "mir2.cloud-readonly-feature-inventory-readback.v2" and inventory.get("ok") is True
            and inventory.get("hostKeyPin") == PIN and inventory.get("hostKeyVerifiedBeforeCredentialInput") is True
            and inventory.get("authenticationConfirmed") is True and inventory.get("remoteExitStatus") == 0, "successful_pinned_inventory_required")
    q = inventory.get("queryResult", {})
    require(q.get("schema") == "mir2.cloud-fresh-readonly-feature-inventory.v2" and q.get("ok") is True
            and q.get("identityStableAcrossRead") is True and q.get("targetBefore") == q.get("targetAfter"), "stable_expected_inventory_required")
    target, binary = q.get("targetBefore", {}), q.get("executable", {})
    require(type(target.get("pid")) is int and target["pid"] > 1 and re.fullmatch("[0-9a-f]{32}", target.get("invocationId", ""))
            and type(target.get("processStartTicks")) is int and target["processStartTicks"] > 0
            and target.get("activeState") == "active" and target.get("subState") == "running"
            and target.get("controlGroup") == "/system.slice/mir2-playtest.service", "expected_target_identity_invalid")
    require(q.get("health", {}).get("revision") == BASELINE and q.get("health", {}).get("ok") is True
            and binary.get("sha256") == BASELINE_BINARY and type(binary.get("bytesHashed")) is int
            and 0 < binary["bytesHashed"] <= 268435456
            and isinstance(binary.get("path"), str)
            and re.fullmatch(r"/opt/mir2-playtest/releases/" + BASELINE + r"-[A-Za-z0-9_-]+/mir2-gateway", binary["path"]), "reviewed_baseline_source_binary_required")
    # Use the same parser on the remote and locally; import executes definitions only.
    namespace = remote_namespace()
    try:
        normalized = namespace["normalized_manifest"](manifest)
    except namespace["AuditError"] as error:
        raise ValueError(str(error)) from None
    require(manifest.get("sourceTree") == manifest.get("sourceRevision", "") + ":" + MAP_TREE
            and len(normalized) == FILE_COUNT and sum(r["bytes"] for r in normalized) == FILE_BYTES, "reviewed_manifest_tree_count_bytes_required")
    return {"target": target, "sourceRevision": BASELINE, "binarySha256": BASELINE_BINARY,
            "binaryBytes": binary["bytesHashed"], "executablePath": binary["path"], "fileCount": FILE_COUNT,
            "totalFileBytes": FILE_BYTES, "normalizedFilesSha256": sha(canonical(normalized))}, normalized


def remote_namespace():
    namespace = {"__name__": "offline_definitions_only"}
    exec(compile(REMOTE, "<readonly-scene-remote>", "exec"), namespace)
    return namespace


def command(config):
    encoded = base64.b64encode(canonical(config)).decode("ascii")
    result = "sudo -n -- python3 -u -c " + shlex.quote(REMOTE) + " " + shlex.quote(encoded)
    require(len(result.encode()) < 131072, "remote_exec_argument_bound")
    return result


def parse_frame(raw):
    frames = [line for line in raw.splitlines(keepends=True) if line.startswith(MARKER)]
    require(len(raw) <= OUTPUT_BOUND and len(frames) == 1 and frames[0].endswith(b"\n"), "single_complete_frame_required")
    value = json.loads(frames[0][len(MARKER):])
    require(isinstance(value, dict) and value.get("schema") == SCHEMA, "remote_schema_mismatch")
    return value


def validate_result(value, config, files):
    expected = config["expected"]
    require(value.get("mapsComplete") is True and value.get("readOnly") is True and value.get("files") == files,
            "complete_exact_map_bytes_required")
    identity = {"pid": expected["target"]["pid"], "invocationId": expected["target"]["invocationId"], "startTicks": expected["target"]["processStartTicks"]}
    require(value.get("identityBefore") == value.get("identityAfter") == identity
            and value.get("sourceRevision") == BASELINE and value.get("binarySha256") == BASELINE_BINARY, "fresh_target_identity_required")
    for key in ["targetBefore", "targetAfter"]:
        frame = value.get(key, {})
        require(frame.get("target") == expected["target"] and frame.get("sourceRevision") == BASELINE
                and frame.get("executable") == {"path": expected["executablePath"], "bytes": expected["binaryBytes"], "sha256": BASELINE_BINARY}
                and frame.get("mapRootSource") == "MIR2_CRYSTAL_MAP_PACK"
                and isinstance(frame.get("mapRoot"), str) and frame["mapRoot"].startswith("/opt/mir2-playtest/")
                and isinstance(frame.get("workingDirectory"), str)
                and remote_namespace()["cwd_allowed"](frame["workingDirectory"]), "fresh_snapshot_invalid")
    require(value["targetBefore"] == value["targetAfter"]
            and value.get("expectedManifestSha256") == config["mapManifestSha256"]
            and value.get("expectedInventorySha256") == config["inventorySha256"]
            and value.get("fileCount") == FILE_COUNT and value.get("totalFileBytes") == FILE_BYTES
            and value.get("normalizedFilesSha256") == expected["normalizedFilesSha256"], "fresh_proof_binding_invalid")
    discovery = value.get("protectedDiscovery", [])
    require(isinstance(discovery, list) and len(discovery) == len(ROLE_HINTS), "all_role_hints_must_be_accounted_for")
    for row, hint in zip(discovery, ROLE_HINTS):
        require(isinstance(row, dict) and row.get("role") == hint["role"] and row.get("pid") == hint["pid"]
                and row.get("status") in ["identified", "unknown_family", "unavailable"]
                and set(row) <= {"role", "pid", "status", "family"}, "anonymous_role_projection_required")
    identified = {r["role"] for r in discovery if r["status"] == "identified"}
    fingerprints = value.get("protectedFingerprintsAfter")
    require(value.get("protectedFingerprintsBefore") == fingerprints and isinstance(fingerprints, list)
            and [f.get("role") for f in fingerprints] == sorted(identified), "protected_process_drift_or_missing_fingerprint")
    for fp in fingerprints:
        require(set(fp) == {"role", "pid", "startTicks", "executableSha256", "commSha256", "cgroupSha256"}
                and fp["pid"] == next(h["pid"] for h in ROLE_HINTS if h["role"] == fp["role"])
                and type(fp["startTicks"]) is int and fp["startTicks"] > 0
                and all(sha_string(fp[key]) for key in ["executableSha256", "commSha256", "cgroupSha256"]), "protected_fingerprint_invalid")
    require(type(value.get("startedAtUnixMs")) is int and type(value.get("atUnixMs")) is int
            and 0 <= value["atUnixMs"] - value["startedAtUnixMs"] <= REMOTE_SECONDS * 1000 + 1000, "actual_read_interval_required")
    require(value.get("serviceOperations") == [] and value.get("configurationChanges") == []
            and value.get("remoteQueryMaxSeconds") == REMOTE_SECONDS, "readonly_scope_required")
    # Unknown VPN cannot create a passed full proof or a fabricated native-status role.
    legacy = {"old-gateway", "old-zone"} <= identified
    roles = {r["role"] for r in ROLE_HINTS} - {"old-gateway", "old-zone"} <= identified
    require(value.get("legacyRolesComplete") is legacy and value.get("protectedRolesComplete") is roles
            and value.get("complete") is (legacy and roles), "complete_protected_roles_required")
    expected_roles = [{"role": r["role"], "pid": r["pid"]} for r in discovery if r["role"] not in ["old-gateway", "old-zone"]] if roles else []
    require(value.get("rolesJSON") == {"roles": expected_roles}, "incomplete_roles_must_not_be_consumable")
    return value


def collect(config):
    """Only called by explicit --collect after all local input and PTY checks."""
    sys.path.insert(0, str(RUNTIME))
    import paramiko
    tcp = transport = channel = timer = None
    password = None
    pin_ok = auth_ok = False
    status = error_class = None
    stage, started = "connect", time.monotonic()
    out, stderr_hash, stderr_size = bytearray(), hashlib.sha256(), 0
    def close_owned():
        for owned in (channel, transport, tcp):
            if owned is not None:
                try:
                    owned.close()
                except Exception:
                    pass
    try:
        tcp = socket.create_connection((HOST, 22), timeout=12)
        transport = paramiko.Transport(tcp)
        transport.get_security_options().key_types = ("ssh-ed25519",)
        transport.banner_timeout = transport.auth_timeout = 12
        transport.start_client(timeout=12)
        stage = "hostKeyPin"
        key = transport.get_remote_server_key()
        fingerprint = "SHA256:" + base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip("=")
        require(key.get_name() == "ssh-ed25519" and hmac.compare_digest(fingerprint, PIN), "host_key_mismatch_before_credential")
        pin_ok = True
        print("Pinned ED25519 host verified before credential input.", flush=True)
        stage = "credentialInput"
        password = getpass.getpass("SSH credential (memory-only, no sudo input): ")
        stage = "authentication"
        transport.auth_password("ubuntu", password, fallback=False)
        password = None
        auth_ok = transport.is_authenticated()
        require(auth_ok, "authentication_not_confirmed")
        stage = "readonlyQuery"
        deadline = time.monotonic() + LOCAL_SECONDS
        timer = threading.Timer(LOCAL_SECONDS, close_owned)
        timer.daemon = True
        timer.start()
        channel = transport.open_session(timeout=12)
        channel.exec_command(command(config))
        channel.shutdown_write()
        channel.settimeout(0)
        while time.monotonic() < deadline:
            for unused in range(4):
                if not channel.recv_stderr_ready():
                    break
                chunk = channel.recv_stderr(32768)
                stderr_size += len(chunk)
                stderr_hash.update(chunk)
            for unused in range(4):
                if not channel.recv_ready():
                    break
                out.extend(channel.recv(32768))
            require(len(out) <= OUTPUT_BOUND and stderr_size <= OUTPUT_BOUND, "diagnostic_output_bound")
            if channel.exit_status_ready() and not channel.recv_ready() and not channel.recv_stderr_ready():
                actual = channel.recv_exit_status()
                status = actual if actual >= 0 else None
                break
            if channel.closed or not transport.is_active():
                break
            time.sleep(.02)
        else:
            raise TimeoutError("local_query_budget")
    except BaseException as error:
        error_class = type(error).__name__
    finally:
        password = None
        if timer is not None:
            timer.cancel()
        close_owned()
    evidence = {"host": HOST, "username": "ubuntu", "hostKeyPin": PIN, "hostKeyVerifiedBeforeCredentialInput": pin_ok,
        "authenticationConfirmed": auth_ok, "remoteExitStatus": status, "remoteExitStatusReceived": status is not None,
        "localErrorClass": error_class, "lastStage": stage, "stdoutBytes": len(out), "stdoutSha256": sha(out),
        "stderrBytes": stderr_size, "stderrSha256": stderr_hash.hexdigest(), "unparsedStdoutPersisted": False,
        "rawStderrPersisted": False, "credentialsPersisted": False, "remoteStdinBytes": 0,
        "remoteQueryMaxSeconds": REMOTE_SECONDS, "localQueryMaxSeconds": LOCAL_SECONDS,
        "elapsedWallMs": round((time.monotonic() - started) * 1000, 3), "remoteCodeSha256": sha(REMOTE.encode()),
        "transportBaselineReaderSha256": TRANSPORT_BASELINE_SHA256, "serviceOperations": [], "configurationChanges": []}
    return bytes(out), evidence


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--collect", action="store_true", help="Explicitly allow this one read-only SSH query")
    parser.add_argument("--expected-inventory", type=Path, required=True)
    parser.add_argument("--expected-inventory-sha256", required=True)
    parser.add_argument("--map-manifest", type=Path, required=True)
    parser.add_argument("--map-manifest-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        require(args.collect and sys.stdin.isatty(), "explicit_collect_and_PTY_required_no_echo_fallback")
        warnings.simplefilter("error", getpass.GetPassWarning)
        inventory = json_input(args.expected_inventory, args.expected_inventory_sha256, 1048576)
        manifest = json_input(args.map_manifest, args.map_manifest_sha256, 1048576)
        expected, files = expectation(inventory, manifest)
        output = args.output
        require(output.is_absolute() and output.drive.lower() == "c:" and not output.exists() and output.parent.is_dir(), "exclusive_new_C_output_required")
        for part in output.parents:
            st = part.lstat()
            require(not part.is_symlink() and not getattr(st, "st_file_attributes", 0) & 0x400, "output_reparse_rejected")
        config = {"expected": expected, "roleHints": ROLE_HINTS, "inventorySha256": args.expected_inventory_sha256, "mapManifestSha256": args.map_manifest_sha256}
        command(config)  # Bound and quote every public remote argument before credential input.
    except Exception as error:
        print(json.dumps({"complete": False, "preflightErrorClass": type(error).__name__}), flush=True)
        return 2
    raw, evidence = collect(config)
    valid = False
    try:
        value = validate_result(parse_frame(raw), config, files)
        valid = evidence["hostKeyVerifiedBeforeCredentialInput"] and evidence["authenticationConfirmed"] and evidence["localErrorClass"] is None and evidence["remoteExitStatus"] == 0 and value.get("complete") is True
    except Exception as error:
        value = {"schema": SCHEMA, "complete": False, "validationErrorClass": type(error).__name__}
        # Persist only the remote error class/code, never arbitrary text or a partial raw frame.
        try:
            failure = parse_frame(raw)
            for key in ["errorClass", "errorCode"]:
                token = failure.get(key)
                if isinstance(token, str) and re.fullmatch("[A-Za-z_][A-Za-z0-9_]{0,99}", token):
                    value[key] = token
        except Exception:
            pass
    value["complete"] = bool(valid)
    value["readerSourceSha256"] = sha(Path(__file__).read_bytes())
    value["transportEvidence"] = evidence
    with output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
    print(json.dumps({"complete": valid, "mapsComplete": value.get("mapsComplete"), "outputSha256": sha(output.read_bytes()), "remoteExitStatus": evidence["remoteExitStatus"]}), flush=True)
    return 0 if valid else 1


if __name__ == "__main__":
    raise SystemExit(main())

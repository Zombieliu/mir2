"""Root-reviewed R20/s16 immutable append. No Gateway changes."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time

STAGE = Path('/srv/.mir2-origin-r20-s16-20261007-01')
PUBLIC = Path('/srv/mir2-client-updates')
LOCK = Path('/var/backups/mir2-client-updates/.publisher.lock')
CONFIG = Path('/etc/caddy/Caddyfile')
GAME = '8faedd89fbcdda7d4f814c05f7d2f56979ab5bf5'
ENGINE = 'b4390ee4c987bbf000ba1d95761151e39fc0d54f'
OLD_MATCH = b'@mir2_native_payload path /client-updates/releases/*'
NEW_MATCH = OLD_MATCH + b' /client-updates/feeds/* /client-updates/installers/*'

def require(value, label):
    if not value:
        raise RuntimeError(label)

def digest(data):
    return hashlib.sha256(data).hexdigest()

def fingerprint(info):
    return (info.st_dev,info.st_ino,info.st_mode,info.st_uid,info.st_nlink,info.st_size,info.st_mtime_ns,info.st_ctime_ns)

def boot():
    require(sys.platform.startswith('linux') and os.geteuid()==0, 'linux-root-required')
    require(Path(__file__).resolve().parent == STAGE, 'fixed-root-stage-required')
    for parent in [*reversed(STAGE.parents),STAGE]:
        info=parent.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid==0 and not info.st_mode & 0o022, 'unsafe-root-stage-parent')
    require(not STAGE.stat().st_mode & 0o077, 'root-stage-not-private')
    path=STAGE/'guards.py'
    before=path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_uid==0 and before.st_nlink==1 and not before.st_mode & 0o077, 'unsafe-guard-file')
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC)
    with os.fdopen(fd,'rb') as stream:
        require(fingerprint(os.fstat(stream.fileno()))==fingerprint(before), 'guard-open-race')
        raw=stream.read(131073)
        require(len(raw)<=131072 and digest(raw)==PINS['guards'], 'guard-source-mismatch')
        require(fingerprint(os.fstat(stream.fileno()))==fingerprint(before) and fingerprint(path.lstat())==fingerprint(before), 'guard-read-race')
    spec=importlib.util.spec_from_file_location('origin_exact_guards',path)
    module=importlib.util.module_from_spec(spec)
    sys.modules[spec.name]=module
    # Execute exactly the bytes just verified, with no second pathname read.
    exec(compile(raw,str(path),'exec'),module.__dict__)
    return module.Guard(),module.Operations()

def write_new(g,ops,path,data,mode=0o600):
    g.parents(path)
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
        os.fchmod(stream.fileno(),mode)
        os.fsync(stream.fileno())
    g.file_info(path)
    ops.sync_dir(g,path.parent)

def snapshot_services():
    names=['caddy.service','mir2-gateway.service','mir2-playtest.service','mir2-periodic-qa.service',
           'mir2-commonware-gateway.service','mir2-admin-api.service','mir2-zone-ucloud.service']
    listeners=subprocess.run(['ss','-Hlntp'],capture_output=True,text=True,check=True).stdout.splitlines()
    connections=subprocess.run(['ss','-Hntp','state','established'],capture_output=True,text=True,check=True).stdout.splitlines()
    states={}
    for name in names:
        result=subprocess.run(['systemctl','show',name,'--property=MainPID,ActiveState'],capture_output=True,text=True,check=True)
        state=dict(line.split('=',1) for line in result.stdout.splitlines() if '=' in line)
        pid=state['MainPID']
        ports={line.split()[3].rsplit(':',1)[-1] for line in listeners if 'pid='+pid+',' in line}
        state['incoming']=sum(1 for line in connections if 'pid='+pid+',' in line and line.split()[2].rsplit(':',1)[-1] in ports)
        states[name]=state
    return states

def same_pids(before,after):
    require(all(before[n]['MainPID']==after[n]['MainPID'] and before[n]['ActiveState']==after[n]['ActiveState'] for n in before),'service-pid-or-state-changed')


PINS = {'candidate': 'WN-CANDIDATE-20261007-invited-20',
 'config': '4f54f9ca961771de96efff9b24d983815cfbbf5c69369bed9da5e9c61dec4632',
 'engineExeSha256': '9213ec3fe1a992f757ea48c53981afa661765b42e637c1ad811e326060b170b8',
 'gameDirectory': 'releases/game-WN-CANDIDATE-20261007-invited-20',
 'guards': '10693760a24973f896caa3f5ac6b1f59e02a32f6e9977cea0dbaa65adbaa0811',
 'new': [{'index': 0,
          'path': 'feeds/s16-fd8109231f6077405c410047fab94ea1a5797e9963e81fd9a80a0cb86cf95dca/latest.json',
          'sha256': 'fd8109231f6077405c410047fab94ea1a5797e9963e81fd9a80a0cb86cf95dca',
          'size': 1159},
         {'index': 1,
          'path': 'feeds/s16-fd8109231f6077405c410047fab94ea1a5797e9963e81fd9a80a0cb86cf95dca/latest.p7s',
          'sha256': '1f881c23724b892ab9814eb2eb9002a365df159a7220bb5316538a0ea181a5c7',
          'size': 1614},
         {'index': 2,
          'path': 'installers/ea1e4b3cbccd9a79cf042d9e0d1d08bcd4e016006df0e3d9f021c9c034d46e03/Mir2Setup.exe',
          'sha256': 'ea1e4b3cbccd9a79cf042d9e0d1d08bcd4e016006df0e3d9f021c9c034d46e03',
          'size': 29245069},
         {'index': 3,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/BUILD-ATTESTATION.json',
          'sha256': '4d3593f85c529640b8aa7a3935a1999c24410a1c671ee250eaeb9b71d14a8182',
          'size': 1493},
         {'index': 4,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/CONTROLS.txt',
          'sha256': 'b9ad2a5a01b3ecbe673a3d38b1974b38492d3a62a0e28932eb8367ce19ebf7c6',
          'size': 10612},
         {'index': 5,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/DELIVERY.json',
          'sha256': '679889dcce106251c5530546f714352dc6d8b03543a3da748d9311b176b0fce2',
          'size': 10560559},
         {'index': 6,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/DELIVERY.p7s',
          'sha256': 'cf4184d9a41537f484441f9316efbd68708cdf57f860c1d937aea4e4e34c6a00',
          'size': 1614},
         {'index': 7,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/KNOWN-ISSUES.md',
          'sha256': 'd2a2ffca1d2170ee0a0f2e3d3fdf0eff644f2fe4472c5ba7e6120ce12c4eadc9',
          'size': 2152},
         {'index': 8,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/MULTILINGUAL-SOURCES.json',
          'sha256': '1528b614dedb07860e22ac2e1cf62528b0ed126230657dc008fb88d2f01f6d11',
          'size': 8618},
         {'index': 9,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/NotoSansTC-OFL.txt',
          'sha256': 'e35ffdebb2225b49814c31889cbb3ce69cb3509e6a9f431edaba5e1a6de8cd15',
          'size': 4481},
         {'index': 10,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/OFL-NotoSans.txt',
          'sha256': 'cee9892f9f0cc8fe882c9e9537ee6a89621d86ee7ceaf70b02e2b2b1c25c061a',
          'size': 4396},
         {'index': 11,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/OFL-NotoSansArabic.txt',
          'sha256': 'a7a5a25eb188bf1cd96982030d53e23c33485c69b1044a562254226857ee13af',
          'size': 4382},
         {'index': 12,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/OFL-NotoSansDevanagari.txt',
          'sha256': 'a216f6f8d85c7228093e0ee5e258d9d377e6671f68acb4db1930b29583d0f331',
          'size': 4386},
         {'index': 13,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/OFL-NotoSansThai.txt',
          'sha256': 'dad6e6abc2bf3fc37cc698af7607c3f4d4235039695713b222e5a034fb5b9b1c',
          'size': 4380},
         {'index': 14,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/PACKAGE-MANIFEST.json',
          'sha256': 'df3fbe3bb4cc44429b8bdeaae4549313878092f8896311c7d84c00d30795b155',
          'size': 31537753},
         {'index': 15,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/README-START.txt',
          'sha256': 'e50595fbc3b973df130d7824d7fb7f91b3b0a46abcf9d2d1888831c065b8902b',
          'size': 49328},
         {'index': 16,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/RELEASE-STATEMENT.json',
          'sha256': '305543a68c651560b484ece5bdc44bed67e4985fe9930e75acf478507baecc84',
          'size': 707},
         {'index': 17,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/RELEASE-STATEMENT.p7s',
          'sha256': '5ad7aced2fdaba635579b1082b8d42ed81d66f80b15c0d371550d04601f94528',
          'size': 1614},
         {'index': 18,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/VERSION.json',
          'sha256': 'f36dfd9df5b1206f173cfd78aea8262bf88c39e8e346e33367982f62ea40b0e6',
          'size': 1221},
         {'index': 19,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/04cd5f15be5159deced5c78129ec8b3c22de13a0fe48492221b0503fd3f3f02a.m2b.gz',
          'sha256': '04cd5f15be5159deced5c78129ec8b3c22de13a0fe48492221b0503fd3f3f02a',
          'size': 63808945},
         {'index': 20,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/07cf86715d833385727589d019dc675860788121749fae2286fe396931109df4.m2d.gz',
          'sha256': '07cf86715d833385727589d019dc675860788121749fae2286fe396931109df4',
          'size': 25562191},
         {'index': 21,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/0c05437ef097c20d60afa63860ecebb023ffb8edbb64ec7015ed90ed2ed59823.m2b.gz',
          'sha256': '0c05437ef097c20d60afa63860ecebb023ffb8edbb64ec7015ed90ed2ed59823',
          'size': 65614809},
         {'index': 22,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/13deda5f6adde99888d88173e70aabfec6497e02ff88f8872d61c1a17bfb3b7d.m2b.gz',
          'sha256': '13deda5f6adde99888d88173e70aabfec6497e02ff88f8872d61c1a17bfb3b7d',
          'size': 55866222},
         {'index': 23,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/3503b719be52717502245782a015acbb7c4aac242e621de21d23b5a3952868ea.m2b.gz',
          'sha256': '3503b719be52717502245782a015acbb7c4aac242e621de21d23b5a3952868ea',
          'size': 60714132},
         {'index': 24,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/35b07f1c78b26f75b14d8bf73b5bddd1fbf88d372e7c65a011a1a0c27c97e9ea.m2b.gz',
          'sha256': '35b07f1c78b26f75b14d8bf73b5bddd1fbf88d372e7c65a011a1a0c27c97e9ea',
          'size': 65602834},
         {'index': 25,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/405448ce67bf1f723b57027d3f2348a53d76657090de5c631739b35c18eb1483.m2b.gz',
          'sha256': '405448ce67bf1f723b57027d3f2348a53d76657090de5c631739b35c18eb1483',
          'size': 55151705},
         {'index': 26,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/565af7b642a235bec7518966243b586bf4668fa93f730e96f747c9400690d2e2.m2b.gz',
          'sha256': '565af7b642a235bec7518966243b586bf4668fa93f730e96f747c9400690d2e2',
          'size': 66861961},
         {'index': 27,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/5bb7e634a19b633d8aca5a7949340a9fc326d1b5afed2a16810b4938e832734c.m2b.gz',
          'sha256': '5bb7e634a19b633d8aca5a7949340a9fc326d1b5afed2a16810b4938e832734c',
          'size': 65547038},
         {'index': 28,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/5efcf2a7a6e22850479f2a007e2565515ae277830e5f61eadc6899e8676128cd.m2b.gz',
          'sha256': '5efcf2a7a6e22850479f2a007e2565515ae277830e5f61eadc6899e8676128cd',
          'size': 12699131},
         {'index': 29,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/6634d75a99bbb0d3486917599b048d07ade067aa7cfc9a980bb09033a513e4c4.m2b.gz',
          'sha256': '6634d75a99bbb0d3486917599b048d07ade067aa7cfc9a980bb09033a513e4c4',
          'size': 60049219},
         {'index': 30,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/84ff6726ef8065d8b18e9ce6a796fb22782be8df1f36a65305fd4ae594d782e7.m2b.gz',
          'sha256': '84ff6726ef8065d8b18e9ce6a796fb22782be8df1f36a65305fd4ae594d782e7',
          'size': 66910012},
         {'index': 31,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/a58dca90e66a5f9b94aa8930dd444712e760dd4ed3e53acd5c9dbcbf527e7bc7.m2b.gz',
          'sha256': 'a58dca90e66a5f9b94aa8930dd444712e760dd4ed3e53acd5c9dbcbf527e7bc7',
          'size': 46858151},
         {'index': 32,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/b405e78cb5ff43098bb3edb1670c367689d96cecc45c0a49c34bafc5b1bd1130.m2d.gz',
          'sha256': 'b405e78cb5ff43098bb3edb1670c367689d96cecc45c0a49c34bafc5b1bd1130',
          'size': 25493950},
         {'index': 33,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/b5f92dd2703b567d7a8dc76124301fb1b39214fc143a8e1431da07d77d553f06.m2b.gz',
          'sha256': 'b5f92dd2703b567d7a8dc76124301fb1b39214fc143a8e1431da07d77d553f06',
          'size': 36317627},
         {'index': 34,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/ba1d18a6fc2ae0e87f4b41ccf730a87e2e4d5f31188957ce348a376b36d08fbe.m2b.gz',
          'sha256': 'ba1d18a6fc2ae0e87f4b41ccf730a87e2e4d5f31188957ce348a376b36d08fbe',
          'size': 59737743},
         {'index': 35,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/delivery/e14bb6681791c37ac1ddb771841db1756b7e9df00df46d240e529cdbb087ceb9.m2b.gz',
          'sha256': 'e14bb6681791c37ac1ddb771841db1756b7e9df00df46d240e529cdbb087ceb9',
          'size': 60049370},
         {'index': 36,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/mir2-client.toml',
          'sha256': '58a5a7cf2ed6e880a7a7f259243dd9ab96150fc43e1a346e6fc79b8de6ee3641',
          'size': 231},
         {'index': 37,
          'path': 'releases/game-WN-CANDIDATE-20261007-invited-20/mir2-platform-windows.exe',
          'sha256': 'e227a68c10042e6f7d7264b2f91a8f252a1b73d5aad4fb6fa92fad3e11d04fcb',
          'size': 111160320},
         {'index': 38,
          'path': 'releases/updater-9213EC3FE1A992F7-s15/ENGINE.json',
          'sha256': '82fed6853aed2ed3d8c8c7520b9d4b84aacd4ac2723c2e47506eb4d001cb99b0',
          'size': 421},
         {'index': 39,
          'path': 'releases/updater-9213EC3FE1A992F7-s15/ENGINE.p7s',
          'sha256': 'c19efee65316762e459ae48afc6123184e4c66defcb33f274fec046c66908a2b',
          'size': 1614},
         {'index': 40,
          'path': 'releases/updater-9213EC3FE1A992F7-s15/Mir2Updater.exe',
          'sha256': '9213ec3fe1a992f757ea48c53981afa661765b42e637c1ad811e326060b170b8',
          'size': 1952256}],
 'prepared': 'de03044efe062b8081502a3976147f40c2e67eef49b8c27ff7a1b48eecbf3c18',
 'previousFeed': {'latest.json': {'sha256': '11a2c8eaaa5f18e654d185223312b18d1c5c32412f844aec64bb341df1c5009c',
                                  'size': 1159},
                  'latest.p7s': {'sha256': '742d8403f6179697d58e5aa5b66c2b45ce4f485bdb573edbe0bf4b9212236ef5',
                                 'size': 1614}},
 'previousGameDirectory': 'releases/game-WN-CANDIDATE-20261007-invited-19',
 'previousObjects': [{'index': 0,
                      'path': 'feeds/s15-11a2c8eaaa5f18e654d185223312b18d1c5c32412f844aec64bb341df1c5009c/latest.json',
                      'sha256': '11a2c8eaaa5f18e654d185223312b18d1c5c32412f844aec64bb341df1c5009c',
                      'size': 1159},
                     {'index': 1,
                      'path': 'feeds/s15-11a2c8eaaa5f18e654d185223312b18d1c5c32412f844aec64bb341df1c5009c/latest.p7s',
                      'sha256': '742d8403f6179697d58e5aa5b66c2b45ce4f485bdb573edbe0bf4b9212236ef5',
                      'size': 1614},
                     {'index': 2,
                      'path': 'installers/ea1e4b3cbccd9a79cf042d9e0d1d08bcd4e016006df0e3d9f021c9c034d46e03/Mir2Setup.exe',
                      'sha256': 'ea1e4b3cbccd9a79cf042d9e0d1d08bcd4e016006df0e3d9f021c9c034d46e03',
                      'size': 29245069},
                     {'index': 3,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/BUILD-ATTESTATION.json',
                      'sha256': '48be8a769aa9ab3c0e8e730f12d481998ebad42a5abbc8202a5ee95299783dd6',
                      'size': 1493},
                     {'index': 4,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/CONTROLS.txt',
                      'sha256': 'b9ad2a5a01b3ecbe673a3d38b1974b38492d3a62a0e28932eb8367ce19ebf7c6',
                      'size': 10612},
                     {'index': 5,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/DELIVERY.json',
                      'sha256': 'b7ba15b5478bff38a04b2678ccb52377e572ac0f7996b7af69403f1d137cf45a',
                      'size': 10560229},
                     {'index': 6,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/DELIVERY.p7s',
                      'sha256': '5d53e88d692f490a27a85404e68f3db1785673037565787cc96276897040e732',
                      'size': 1614},
                     {'index': 7,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/KNOWN-ISSUES.md',
                      'sha256': 'd2a2ffca1d2170ee0a0f2e3d3fdf0eff644f2fe4472c5ba7e6120ce12c4eadc9',
                      'size': 2152},
                     {'index': 8,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/MULTILINGUAL-SOURCES.json',
                      'sha256': '1528b614dedb07860e22ac2e1cf62528b0ed126230657dc008fb88d2f01f6d11',
                      'size': 8618},
                     {'index': 9,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/NotoSansTC-OFL.txt',
                      'sha256': 'e35ffdebb2225b49814c31889cbb3ce69cb3509e6a9f431edaba5e1a6de8cd15',
                      'size': 4481},
                     {'index': 10,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/OFL-NotoSans.txt',
                      'sha256': 'cee9892f9f0cc8fe882c9e9537ee6a89621d86ee7ceaf70b02e2b2b1c25c061a',
                      'size': 4396},
                     {'index': 11,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/OFL-NotoSansArabic.txt',
                      'sha256': 'a7a5a25eb188bf1cd96982030d53e23c33485c69b1044a562254226857ee13af',
                      'size': 4382},
                     {'index': 12,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/OFL-NotoSansDevanagari.txt',
                      'sha256': 'a216f6f8d85c7228093e0ee5e258d9d377e6671f68acb4db1930b29583d0f331',
                      'size': 4386},
                     {'index': 13,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/OFL-NotoSansThai.txt',
                      'sha256': 'dad6e6abc2bf3fc37cc698af7607c3f4d4235039695713b222e5a034fb5b9b1c',
                      'size': 4380},
                     {'index': 14,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/PACKAGE-MANIFEST.json',
                      'sha256': '71277a8af15632a715d5e8d72ca95004fbee9a687ad4f963df8016e5368897ba',
                      'size': 31537753},
                     {'index': 15,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/README-START.txt',
                      'sha256': '720a139384723b1211d8b6efb1caa18d7d9062b1bca8a8f8e93f0f13de3f1551',
                      'size': 49328},
                     {'index': 16,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/RELEASE-STATEMENT.json',
                      'sha256': 'e614b21b5ffa3ce82dfaf8c39270f317a3defed0bf0c1c378b52637b03ac0b24',
                      'size': 707},
                     {'index': 17,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/RELEASE-STATEMENT.p7s',
                      'sha256': 'd8a9d7162e4c851fa66b7d86cc206621f514e7348da28e933b80139932bfd3b1',
                      'size': 1614},
                     {'index': 18,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/VERSION.json',
                      'sha256': '66e60b50a181511acb7a9076c959d1d23c9d890d9e2fa6caa168e42ccbf52681',
                      'size': 1221},
                     {'index': 19,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/0335ad7cdf9c3ff44fb3bee101e8f50fd839a4796be9a5fddbf7822fd680e7e0.m2b.gz',
                      'sha256': '0335ad7cdf9c3ff44fb3bee101e8f50fd839a4796be9a5fddbf7822fd680e7e0',
                      'size': 60714170},
                     {'index': 20,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/252ff6897b3b3813badb123752b463af8e1c068a57cb6a11ffe9261a07b66d87.m2b.gz',
                      'sha256': '252ff6897b3b3813badb123752b463af8e1c068a57cb6a11ffe9261a07b66d87',
                      'size': 63807337},
                     {'index': 21,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/3dee530c49de40dfad3afb3b7b83f0d9426fa0ac202202567f2c01f20ea21a38.m2b.gz',
                      'sha256': '3dee530c49de40dfad3afb3b7b83f0d9426fa0ac202202567f2c01f20ea21a38',
                      'size': 46880754},
                     {'index': 22,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/53a489c9be927fbad4087d595567604c6cfa33a4960bb48eba646a631203974b.m2b.gz',
                      'sha256': '53a489c9be927fbad4087d595567604c6cfa33a4960bb48eba646a631203974b',
                      'size': 65601792},
                     {'index': 23,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/565af7b642a235bec7518966243b586bf4668fa93f730e96f747c9400690d2e2.m2b.gz',
                      'sha256': '565af7b642a235bec7518966243b586bf4668fa93f730e96f747c9400690d2e2',
                      'size': 66861961},
                     {'index': 24,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/5efcf2a7a6e22850479f2a007e2565515ae277830e5f61eadc6899e8676128cd.m2b.gz',
                      'sha256': '5efcf2a7a6e22850479f2a007e2565515ae277830e5f61eadc6899e8676128cd',
                      'size': 12699131},
                     {'index': 25,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/6634d75a99bbb0d3486917599b048d07ade067aa7cfc9a980bb09033a513e4c4.m2b.gz',
                      'sha256': '6634d75a99bbb0d3486917599b048d07ade067aa7cfc9a980bb09033a513e4c4',
                      'size': 60049219},
                     {'index': 26,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/6fb1307f7539f7454f9552ad2639f6e8ef3f75344c958b6ede94207641301a4e.m2b.gz',
                      'sha256': '6fb1307f7539f7454f9552ad2639f6e8ef3f75344c958b6ede94207641301a4e',
                      'size': 65556826},
                     {'index': 27,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/74769fd0ba6af20b4c9f7a7b66a08890590772c5854d84466429d39b12b16722.m2b.gz',
                      'sha256': '74769fd0ba6af20b4c9f7a7b66a08890590772c5854d84466429d39b12b16722',
                      'size': 55867713},
                     {'index': 28,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/84ff6726ef8065d8b18e9ce6a796fb22782be8df1f36a65305fd4ae594d782e7.m2b.gz',
                      'sha256': '84ff6726ef8065d8b18e9ce6a796fb22782be8df1f36a65305fd4ae594d782e7',
                      'size': 66910012},
                     {'index': 29,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/a0ede1b679d255a2dcfe5c22269036e105b321a618b3c0f122fc2bbe468ced11.m2d.gz',
                      'sha256': 'a0ede1b679d255a2dcfe5c22269036e105b321a618b3c0f122fc2bbe468ced11',
                      'size': 25543855},
                     {'index': 30,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/ba1d18a6fc2ae0e87f4b41ccf730a87e2e4d5f31188957ce348a376b36d08fbe.m2b.gz',
                      'sha256': 'ba1d18a6fc2ae0e87f4b41ccf730a87e2e4d5f31188957ce348a376b36d08fbe',
                      'size': 59737743},
                     {'index': 31,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/c407ba4eaf8b686ef8ccdaf7d8a8e4d41a98df2c75b9270850940d0de3c5b60f.m2b.gz',
                      'sha256': 'c407ba4eaf8b686ef8ccdaf7d8a8e4d41a98df2c75b9270850940d0de3c5b60f',
                      'size': 36299805},
                     {'index': 32,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/cb84a498f2e024905c52930a5c07018d168fe58279550ed5815536aabae63e4c.m2b.gz',
                      'sha256': 'cb84a498f2e024905c52930a5c07018d168fe58279550ed5815536aabae63e4c',
                      'size': 55134787},
                     {'index': 33,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/e14bb6681791c37ac1ddb771841db1756b7e9df00df46d240e529cdbb087ceb9.m2b.gz',
                      'sha256': 'e14bb6681791c37ac1ddb771841db1756b7e9df00df46d240e529cdbb087ceb9',
                      'size': 60049370},
                     {'index': 34,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/delivery/e83e50787df75d6e3d4a64aef7640e0d36b849e17aaae87568f8a1d526d6b25b.m2b.gz',
                      'sha256': 'e83e50787df75d6e3d4a64aef7640e0d36b849e17aaae87568f8a1d526d6b25b',
                      'size': 65610644},
                     {'index': 35,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/mir2-client.toml',
                      'sha256': '58a5a7cf2ed6e880a7a7f259243dd9ab96150fc43e1a346e6fc79b8de6ee3641',
                      'size': 231},
                     {'index': 36,
                      'path': 'releases/game-WN-CANDIDATE-20261007-invited-19/mir2-platform-windows.exe',
                      'sha256': '3abc0c7b74af234d797705ac9ce32a68abefac70bf43095093a353c81145f297',
                      'size': 111107072},
                     {'index': 37,
                      'path': 'releases/updater-9213EC3FE1A992F7-s15/ENGINE.json',
                      'sha256': '82fed6853aed2ed3d8c8c7520b9d4b84aacd4ac2723c2e47506eb4d001cb99b0',
                      'size': 421},
                     {'index': 38,
                      'path': 'releases/updater-9213EC3FE1A992F7-s15/ENGINE.p7s',
                      'sha256': 'c19efee65316762e459ae48afc6123184e4c66defcb33f274fec046c66908a2b',
                      'size': 1614},
                     {'index': 39,
                      'path': 'releases/updater-9213EC3FE1A992F7-s15/Mir2Updater.exe',
                      'sha256': '9213ec3fe1a992f757ea48c53981afa661765b42e637c1ad811e326060b170b8',
                      'size': 1952256}],
 'previousProof': '099702223680a43d63fb500740763e8b8f67442c6fcd414b6d5f178293d7d7a3',
 'previousPublication': '852b474af0f9c7a5512b1ea1b3d9d2c957cc512dec95e366b8907052641c2356',
 'proof': '6a59813e8e3451820a064e1ff1c9f7525f77e0aab6ed85ffa791c8679c3192e8',
 'publication': '905bc48ea9a210d5cb8ef5d82f748b341d9654cf287f1c3803ce74a1a7ae556d',
 'sourceNames': {0: '00-latest.json',
                 1: '01-latest.p7s',
                 2: '02-Mir2Setup.exe',
                 3: '03-BUILD-ATTESTATION.json',
                 4: '04-CONTROLS.txt',
                 5: '05-DELIVERY.json',
                 6: '06-DELIVERY.p7s',
                 7: '07-KNOWN-ISSUES.md',
                 8: '08-MULTILINGUAL-SOURCES.json',
                 9: '09-NotoSansTC-OFL.txt',
                 10: '10-OFL-NotoSans.txt',
                 11: '11-OFL-NotoSansArabic.txt',
                 12: '12-OFL-NotoSansDevanagari.txt',
                 13: '13-OFL-NotoSansThai.txt',
                 14: '14-PACKAGE-MANIFEST.json',
                 15: '15-README-START.txt',
                 16: '16-RELEASE-STATEMENT.json',
                 17: '17-RELEASE-STATEMENT.p7s',
                 18: '18-VERSION.json',
                 19: '19-04cd5f15be5159deced5c78129ec8b3c22de13a0fe48492221b0503fd3f3f02a.m2b.gz',
                 20: '20-07cf86715d833385727589d019dc675860788121749fae2286fe396931109df4.m2d.gz',
                 21: '21-0c05437ef097c20d60afa63860ecebb023ffb8edbb64ec7015ed90ed2ed59823.m2b.gz',
                 22: '22-13deda5f6adde99888d88173e70aabfec6497e02ff88f8872d61c1a17bfb3b7d.m2b.gz',
                 23: '23-3503b719be52717502245782a015acbb7c4aac242e621de21d23b5a3952868ea.m2b.gz',
                 24: '24-35b07f1c78b26f75b14d8bf73b5bddd1fbf88d372e7c65a011a1a0c27c97e9ea.m2b.gz',
                 25: '25-405448ce67bf1f723b57027d3f2348a53d76657090de5c631739b35c18eb1483.m2b.gz',
                 26: '26-565af7b642a235bec7518966243b586bf4668fa93f730e96f747c9400690d2e2.m2b.gz',
                 27: '27-5bb7e634a19b633d8aca5a7949340a9fc326d1b5afed2a16810b4938e832734c.m2b.gz',
                 28: '28-5efcf2a7a6e22850479f2a007e2565515ae277830e5f61eadc6899e8676128cd.m2b.gz',
                 29: '29-6634d75a99bbb0d3486917599b048d07ade067aa7cfc9a980bb09033a513e4c4.m2b.gz',
                 30: '30-84ff6726ef8065d8b18e9ce6a796fb22782be8df1f36a65305fd4ae594d782e7.m2b.gz',
                 31: '31-a58dca90e66a5f9b94aa8930dd444712e760dd4ed3e53acd5c9dbcbf527e7bc7.m2b.gz',
                 32: '32-b405e78cb5ff43098bb3edb1670c367689d96cecc45c0a49c34bafc5b1bd1130.m2d.gz',
                 33: '33-b5f92dd2703b567d7a8dc76124301fb1b39214fc143a8e1431da07d77d553f06.m2b.gz',
                 34: '34-ba1d18a6fc2ae0e87f4b41ccf730a87e2e4d5f31188957ce348a376b36d08fbe.m2b.gz',
                 35: '35-e14bb6681791c37ac1ddb771841db1756b7e9df00df46d240e529cdbb087ceb9.m2b.gz',
                 36: '36-mir2-client.toml',
                 37: '37-mir2-platform-windows.exe',
                 38: '38-ENGINE.json',
                 39: '39-ENGINE.p7s',
                 40: '40-Mir2Updater.exe'}}

def ensure_parent(g,ops,path):
    require(path.is_relative_to(PUBLIC) and '..' not in path.parts,'target-escape')
    relative=path.relative_to(PUBLIC)
    current=PUBLIC
    for part in relative.parts:
        require(re.fullmatch(r'[A-Za-z0-9_.-]+',part) and part not in ('.','..'),'target-component')
        current=current/part
        if not current.exists() and not current.is_symlink():
            g.directory(current.parent)
            try:
                os.mkdir(current,0o755)
                ops.sync_dir(g,current.parent)
            except FileExistsError:
                pass
        g.directory(current)

def check_target(g,obj):
    target=PUBLIC/obj['path']
    require(target.is_relative_to(PUBLIC) and '..' not in Path(obj['path']).parts and not Path(obj['path']).is_absolute(),'target-escape')
    # Every existing ancestor is checked even for an absent leaf.
    current=PUBLIC
    g.directory(current)
    for part in Path(obj['path']).parts[:-1]:
        current=current/part
        if current.exists() or current.is_symlink():
            g.directory(current)
        else:
            break
    if target.exists() or target.is_symlink():
        g.match(target,obj)
        require(stat.S_IMODE(g.file_info(target).st_mode)==0o444,'new-target-not-public-immutable')
        return 'exact-existing'
    return 'absent'

def stable_feed(g):
    identities = {}
    for name, pin in PINS['previousFeed'].items():
        path = PUBLIC / 'feed-current' / name
        raw = g.read(path, 32768, pin)
        identities[name] = g.identity(g.file_info(path))
        if name == 'latest.json':
            value = json.loads(raw)
            require(value['sequence'] == 15 and value['game']['directory'] == PINS['previousGameDirectory'], 'feed15-game-binding')
            require(value['expiresUnix'] >= int(time.time()), 'feed15-expired')
    return identities


def fixed_envelope(g):
    values = {}
    for name, key in [('ROOT-VERIFICATION.json', 'proof'), ('PUBLICATION-PLAN.json', 'publication'),
                      ('APPEND-INPUTS.prepared.json', 'prepared'), ('PREVIOUS-ROOT-VERIFICATION.json', 'previousProof'),
                      ('PREVIOUS-PUBLICATION-PLAN.json', 'previousPublication')]:
        raw = g.read(STAGE / name, 32 * 1024 * 1024)
        require(digest(raw) == PINS[key], 'envelope-hash:' + name)
        values[name] = json.loads(raw)
    for previous, plan_name, proof_name, sequence in [
        (False, 'PUBLICATION-PLAN.json', 'ROOT-VERIFICATION.json', 16),
        (True, 'PREVIOUS-PUBLICATION-PLAN.json', 'PREVIOUS-ROOT-VERIFICATION.json', 15)]:
        proof = values[proof_name]
        require(proof['passed'] is True and proof['cmsVerified'] is True and proof['candidateVerified'] is True
                and proof['sequence'] == sequence and proof['sourceRevision'] == ENGINE, 'genuine-root-proof-binding')
        publication = values[plan_name]
        require(publication['verificationReceipt']['sha256'] == PINS['previousProof' if previous else 'proof'], 'plan-proof-binding')
        objects = publication['objects']
        pins = PINS['previousObjects' if previous else 'new']
        require(len(objects) == len(pins) and len({o['path'] for o in objects}) == len(pins), 'fixed-closed-object-count')
        closure = sorted([{k: o[k] for k in ('path', 'size', 'sha256')} for o in objects], key=lambda o: o['path'])
        canonical = (json.dumps(closure, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode()
        require(digest(canonical) == proof['objectsSha256'], 'closed-object-proof-digest')
        selected = {o['path']: o for o in objects}
        for pin in pins:
            bound = selected[pin['path']]
            require(all(bound[k] == pin[k] for k in ('size', 'sha256')), 'fixed-object-proof-mismatch')
    prepared = values['APPEND-INPUTS.prepared.json']
    require(prepared['gameSource'] == GAME and prepared['engineSource'] == ENGINE and prepared['sequence'] == 16,
            'prepared-source-binding')
    feed = json.loads(g.read(STAGE / PINS['sourceNames'][0], 32768, PINS['new'][0]))
    require(feed['sequence'] == 16 and feed['expiresUnix'] >= int(time.time()) and
            feed['game']['directory'] == PINS['gameDirectory'] and feed['game']['identity'] == PINS['candidate'],
            'feed16-expired-or-mismatch')
    statement_pin = next(o for o in PINS['new'] if o['path'] == PINS['gameDirectory'] + '/RELEASE-STATEMENT.json')
    statement = json.loads(g.read(STAGE / PINS['sourceNames'][statement_pin['index']], 32768, statement_pin))
    require(statement['gitRevision'] == GAME and statement['candidate'] == PINS['candidate']
            and statement['worktreeDirty'] is False, 'game-statement-source-binding')
    engine_pin = next(o for o in PINS['new'] if o['path'] == feed['engine']['directory'] + '/ENGINE.json')
    engine = json.loads(g.read(STAGE / PINS['sourceNames'][engine_pin['index']], 32768, engine_pin))
    require(engine['sourceRevision'] == ENGINE and engine['exeSha256'].lower() == PINS['engineExeSha256'], 'reused-engine-binding')
    return values['PUBLICATION-PLAN.json']['objects']


def copy_frozen(g, ops, source, target, pin, mode):
    g.parents(target)
    with g.opened(source) as (reader, before):
        require(before.st_size == pin['size'], 'copy-source-size')
        fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
        with os.fdopen(fd, 'wb') as writer:
            count, hasher = 0, hashlib.sha256()
            while chunk := reader.read(1024 * 1024):
                count += len(chunk)
                require(count <= pin['size'], 'copy-source-overrun')
                hasher.update(chunk)
                writer.write(chunk)
            require(count == pin['size'] and hasher.hexdigest() == pin['sha256'], 'copy-source-hash')
            writer.flush()
            os.fsync(writer.fileno())
            os.fchmod(writer.fileno(), mode)
            os.fsync(writer.fileno())
    g.match(target, pin)
    ops.sync_dir(g, target.parent)


def append(g, ops, objects, feed_before):
    baseline = {o['path']: g.match(PUBLIC / o['path'], o) for o in PINS['previousObjects']}
    for obj in PINS['new']:
        g.match(STAGE / PINS['sourceNames'][obj['index']], obj)
        check_target(g, obj)
    require(stable_feed(g) == feed_before, 'feed-changed-before-append')
    events = []
    for obj in reversed(PINS['new']):
        index = obj['index']
        status = check_target(g, obj)
        if status == 'absent':
            ensure_parent(g, ops, (PUBLIC / obj['path']).parent)
            original = STAGE / PINS['sourceNames'][index]
            staged = STAGE / f'append-{index:02}'
            if staged.exists() or staged.is_symlink():
                g.match(staged, obj)
                require(stat.S_IMODE(g.file_info(staged).st_mode) == 0o444, 'staged-mode-conflict')
            else:
                copy_frozen(g, ops, original, staged, obj, 0o444)
            require(stable_feed(g) == feed_before, 'feed-changed-during-append')
            try:
                ops.rename_new(g, staged, PUBLIC / obj['path'])
                status = 'created'
            except FileExistsError:
                check_target(g, obj)
                status = 'raced-exact-existing'
        check_target(g, obj)
        require(stable_feed(g) == feed_before, 'feed-changed-after-append')
        event = {'index': index, 'path': obj['path'], 'status': status, 'size': obj['size'], 'sha256': obj['sha256']}
        audit = STAGE / f'APPEND-EVENT-{index:02}.json'
        if audit.exists() or audit.is_symlink():
            previous = json.loads(g.read(audit, 32768))
            require(all(previous[k] == event[k] for k in ('index', 'path', 'size', 'sha256')) and
                    previous['status'] in ('created', 'exact-existing', 'raced-exact-existing'), 'append-audit-conflict')
        else:
            write_new(g, ops, audit, (json.dumps(event, sort_keys=True) + '\n').encode())
        events.append(event)
    for obj in objects:
        g.match(PUBLIC / obj['path'], obj)
    for obj in PINS['previousObjects']:
        require(g.match(PUBLIC / obj['path'], obj) == baseline[obj['path']], 'previous-object-fingerprint-changed')
    return events


def main():
    require(len(sys.argv) == 2 and sys.argv[1] in ('preflight', 'append'), 'fixed-operation-required')
    mode = sys.argv[1]
    g, ops = boot()
    g.directory(PUBLIC)
    g.directory(STAGE, private=True)
    require(STAGE.stat().st_dev == PUBLIC.stat().st_dev, 'stage-filesystem-mismatch')
    states = snapshot_services()
    with ops.lock(g, LOCK):
        feed_before = stable_feed(g)
        require(digest(g.read(CONFIG, 128 * 1024)) == PINS['config'], 'caddy-preflight-conflict')
        objects = fixed_envelope(g)
        result = {'schema': 'mir2.origin-r20-s16.fixed-closure.v1', 'operation': mode, 'gameSource': GAME,
                  'engineSource': ENGINE, 'sequence': 16, 'objects': len(objects), 'rootProofSha256': PINS['proof'],
                  'createdUnix': int(time.time()), 'changedCurrentFeed': False, 'gameServiceRestart': False}
        if mode == 'append':
            result['events'] = append(g, ops, objects, feed_before)
        else:
            for obj in PINS['previousObjects']:
                g.match(PUBLIC / obj['path'], obj)
            result['targets'] = [{'path': o['path'], 'status': check_target(g, o)} for o in objects]
            for obj in PINS['new']:
                g.match(STAGE / PINS['sourceNames'][obj['index']], obj)
        require(stable_feed(g) == feed_before, 'feed-final-conflict')
        require(digest(g.read(CONFIG, 128 * 1024)) == PINS['config'], 'caddy-final-conflict')
        same_pids(states, snapshot_services())
        result.update(passed=True, feed15FingerprintPreserved=True, allServicePidsPreserved=True)
        raw = (json.dumps(result, indent=2) + '\n').encode()
        write_new(g, ops, STAGE / f'{mode.upper()}-01.json', raw)
        print(raw.decode(), end='')


if __name__ == '__main__':
    main()

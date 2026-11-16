from pathlib import Path
import hashlib
import json
import subprocess
import sys

work = Path(__file__).resolve().parent
repo = work.parent.parent
parent = '78d3abecb887d0e2c447d06e8224e7670486758e'
qa = repo / 'mir2-web3/docs/generated/slg-production-p02b2c-20261009'
review_path = work / 'independent-result-review.json'
review_sha = hashlib.sha256(review_path.read_bytes()).hexdigest()
assert len(sys.argv) == 2 and sys.argv[1] == review_sha, 'Independent review pin required'
review = json.loads(review_path.read_text(encoding='utf-8'))
audit = json.loads((work / 'result-audit-01.json').read_text(encoding='utf-8'))
raw = json.loads((qa / 'raw-evidence.json').read_text(encoding='utf-8'))
assert review['accepted'] and not review['discrepancies']
counts = review['testCounts']
assert counts['total'] == audit['actualPassingExecutions']
assert counts['unique'] == audit['distinctNamedPassingTests']
assert counts['new'] == audit['newUniqueTests'] == 8
assert counts['failed'] == counts['ignored'] == 0
total, unique = counts['total'], counts['unique']
review_target = qa / review_path.name
assert not review_target.exists(), 'Preserve existing review copy'
review_target.write_bytes(review_path.read_bytes())
assert hashlib.sha256(review_target.read_bytes()).hexdigest() == review_sha
(qa / 'independent-result-review-copy.json').write_text(json.dumps({
    'schema': 'mir2.production-review-copy.v1', 'source': str(review_path),
    'destination': str(review_target), 'sha256': review_sha,
    'length': review_target.stat().st_size, 'rawEvidenceManifestEntries': raw['rawCopies'],
    'includedInEarlierRawManifest': False,
}, indent=2) + '\n', encoding='utf-8')

run_counts = '＋'.join(str(item['actualPassingExecutions']) for item in audit['checks'])
header = f'''## 制作加工 P02b-2c：宠物奖励箱可信发行，限定回归通过（2026-10-09）

BlackStone 与 Strongbox 使用同一可信 File 物品编号服务。先验证源箱子、刷新仍含源箱子的完整历史，再准备奖励；有效完整载体取得新编号后，箱子消耗和奖励一起发布。服务缺失、冻结、损坏、耗尽或配置不匹配时，箱子和背包保持原状，不发送成功或领奖包。原版空奖励与空间不足的箱子消耗行为、正常随机奖励及既有快捷栏回退保持。

最终原串行 CargoGuard：新奖励箱、召唤物、道具使用、File编号与保存回归，{run_counts}＝{total}次通过，{unique}个不同具名测试，8项新增，0失败/ignored。五次 Source02 调用的462个声明输入前后及最终匹配，458个受保护输入与生产父提交78d3abe匹配；4个作者输入含一处旧回城卷测试夹具。原工具、50GiB/2000ms门槛及PolicyB保持。Source01的8/8、55/55和原道具回归46通过/1失败作为历史保留，不叠加通过数；后者旧夹具已绑定真实安全区却断言出生点，只显式固定该夹具绑定点，所有原断言和实际回城规则保持。

本批仅完成奖励箱个人同步发放与File UID保护；重登测试使用内存账号库，不是完整账号原子持久事务。周期黑石邮件、领取保管身份、其他发行者、全量历史盘点、Postgres及未知提交协调仍开放。原背包满时检查与快捷栏回退的差异未在本批扩大修改。默认绑定、公共配方、正常网关与安装包未切换，未执行UI/玩家验收，不提高整体parity验收比例。详见[实现进度](SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md)与[本批原始验证](generated/slg-production-p02b2c-20261009/README.md)。以下各批为历史。

'''
old_queue = '- [ ] P02b-2：其余发行者（通用add/normalize、特殊任务/脚本、GM、邮件/商城/宠物兑换、初始种子及Hero）继续迁移；补掉落生成时机、部分丢弃来源与共享持久认领，完整保管返回保留原UID并拒绝冲突。'
new_queue = f'''- [x] P02b-2c：受控File个人 BlackStone/Strongbox 奖励箱先抬源历史、验证载体并取得新UID，再同步发布箱子消耗与奖励；服务失败保留箱子，原空奖励语义保持。8项新增、{total}次/{unique}不同测试通过；内存账号重登不代表完整持久事务。
- [ ] P02b-2：其余发行者（通用add/normalize、特殊任务/脚本、GM、邮件/商城、周期黑石邮件及领取/宠物兑换、初始种子及Hero）继续迁移；补掉落生成时机、部分丢弃来源与共享持久认领，完整保管返回保留原UID并拒绝冲突。'''

paths = ['AGENT-TASK-QUEUE.md', 'CRYSTAL-1TO1-ROADMAP.md', 'BACKEND-1TO1-PROGRESS.md', 'CRYSTAL-SERVER-PARITY.md']
scoped = work / 'scoped-docs'
assert not scoped.exists(), 'Preserve existing scoped documents'
scoped.mkdir()
rows = []


def prepend(text):
    first, rest = text.replace('\r\n', '\n').split('\n', 1)
    assert '## 制作加工 P02b-2c：' not in rest, 'Do not duplicate progress'
    return (first + '\n\n' + header + rest.lstrip('\n')).replace(old_queue, new_queue)


for name in paths:
    relative = f'mir2-web3/docs/{name}'
    path = repo / relative
    before = path.read_bytes()
    current = before.decode('utf-8')
    if name == 'AGENT-TASK-QUEUE.md':
        assert old_queue in current, 'Expected pending queue entry missing'
    base = subprocess.check_output(['git', '-c', f'safe.directory={repo}', '-c', 'gc.auto=0', '-C', str(repo), 'show', f'{parent}:{relative}']).decode('utf-8')
    assert path.read_bytes() == before, 'Concurrent foreign document change'
    path.write_text(prepend(current), encoding='utf-8', newline='\n')
    target = scoped / name
    target.write_text(prepend(base), encoding='utf-8', newline='\n')
    rows.append({'path': relative, 'scopedPath': str(target), 'sha256': hashlib.sha256(target.read_bytes()).hexdigest(), 'preservedForeignCheckoutContent': True})
(work / 'scoped-docs.json').write_text(json.dumps(rows, indent=2) + '\n', encoding='utf-8')

implementation = repo / 'mir2-web3/docs/SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md'
before = implementation.read_bytes()
text = before.decode('utf-8').replace('\r\n', '\n')
assert '## P02b-2c' not in text
marker = '## P02b-2b 个人掉落拾取与保管身份'
assert text.count(marker) == 1
section = f'''## P02b-2c 宠物奖励箱发行

BlackStone（shape21）及 Strongbox（shape25）现在经同一可信File权威发行新的完整奖励载体。源引用匹配后、源箱子仍在Live World时先抬历史下限，避免空奖励或移除后重用其编号；正常选择、随机属性和WonderDrug完整数据保持。奖励验证并获得新UID后，一次替换库存发布箱子消耗与成品；不可用、冻结、损坏、耗尽或错配服务失败不消耗箱子、不发成功包，绝不退回局部编号。

新增8项实际UseItem/受控引用测试覆盖高UID来源退休、完整载体与内存账号重登、共享服务的两个会话、损坏/耗尽/冻结/错配、真实空奖励表和旧引用。原空奖励成功消耗与原NoSpace消耗语义保留，快捷栏回退的原版差异仍待单独对齐。周期黑石产出与邮件领取不在本批，不能据此宣称宠物全部发行者已迁移。

原串行CargoGuard最终 **{total}次通过、{unique}个不同具名测试、8项新增、0失败/ignored**；462个声明输入前后及最终一致、458个受保护依赖与父提交78d3abe一致。原工具、50GiB/2000ms与PolicyB保持。旧回城卷测试因默认安全区绑定与出生点假设不一致，Source01道具回归46/1失败完整保留；Source02只固定该夹具绑定点，所有原断言和游戏回城规则保持。先前Source01正向检查不叠加计入最终结果。见[P02b-2c验证](generated/slg-production-p02b2c-20261009/README.md)。

仍仅是File个人同步发放，不是完整账号原子持久、共享保管认领、全部发行者、历史盘点、Postgres或未知提交协调验收。默认绑定和公共目录保持禁用，没有NPC/网关新入口或安装包切换；采集、种植、作坊、组合建材/食品和侧栏管理尚待落地。

'''
assert implementation.read_bytes() == before
implementation.write_text(text.replace(marker, section + marker), encoding='utf-8', newline='\n')

table = '\n'.join(f"| {item['run']} | {item['actualPassingExecutions']} | 0 / 0 |" for item in audit['checks'])
readme = f'''# Personal creature reward box issuance — 2026-10-09

| Final actual original CargoGuard call | Passing executions | Failed / ignored |
| --- | ---: | --- |
{table}

**{total} passing executions; {unique} distinct fully qualified names; 8 new tests.** Repeated names across filters are not extra scenarios. All8 new names occur in actual original stdout. Original1.95.0 Cargo, locked/offline/jobs1, nonincremental compilation and serial test threads remain. The462 declared bounded simulation inputs match before/after every final call and final audit; all458 protected canonical Git-clean blobs match production parent78d3abe. Four authored paths match Source02, including only one original test fixture insertion. This is not whole-workspace qualification.

Original Guard/source, license, Cargo/rustup, PowerShell, probe, policy and source/authority bindings were independently checked. Every actual run has its original nonce and PolicyB completed/exited/disposed lifecycle. Original53687091200-byte and2000ms gates remain; observed timing does not promise a deadline. [Independent actual review](independent-result-review.json).

`raw-evidence.json` records{raw['rawCopies']} exact-byte copies, including all{raw['actualOriginalGuardNonces']} actual Guard calls and their original nonce files, raw logs/policies/source snapshots, source01/02 manifests, archives/reviews, helpers and audit history. The later final review is separately byte-checked by `independent-result-review-copy.json`, outside that earlier manifest. `.gitattributes` preserves raw bytes including empty stderr files; compiled caches are not included.

Historical Source01 focus8/8 and creature55/55 remain as historical positive checks. Source01 use-item had46pass/1fail: the original routing fixture asserted configured spawn despite default bootstrap having a Crystal safe-zone bind. Source02 explicitly pins this fixture bind point; original exact position, UserLocation, UseItem and inventory assertions remain byte-for-byte after removing that insertion. Product teleport behavior is unchanged. Original tested tests.rs bytes are archived, and no history is added to final passing totals.

The box source remains live for original-world history refresh before retirement. Shared trusted File authority issues the valid frozen reward and one synchronous inventory publication commits source removal and reward. Corruption/exhaustion/fences/mismatch preserve the box and inventory. Legacy random/stat/dynamic reward selection and source-avoid allocation, empty result and NoSpace semantics are retained. Existing any-Belt fallback differs from Crystal's Inventory gate and remains separately open. Normal UseItem and logout/relogin use the existing memory AccountStore in these tests.

**Accepted scope: controlled File authority and personal synchronous inventory publication only.** Not full durable account atomic persistence, periodic BlackCreatureStone mail, custody claim identity, all-issuer migration, PostgreSQL, census/reconciliation, or public gameplay. Default bindings/catalog/normal gateway remain disabled. No game/GUI/socket/server/database/installer was started or published. Gathering, crops, workshops, construction/food and selected side-management UI are subsequent work. [Implementation progress](../../SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md).
'''
(qa / 'README.md').write_text(readme, encoding='utf-8', newline='\n')
print(json.dumps({'updatedProgressDocs': paths, 'reviewSha256': review_sha, 'passingExecutions': total, 'distinctTests': unique, 'normalCatalogEnabled': False}))

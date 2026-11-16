"""Copy only existing isolated fixture helpers; append focused terminal checks."""
from pathlib import Path
import hashlib
import json

OUT = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/gn/mir2-player-journey/mir2-web3')
source = ROOT / 'apps/gateway/tests/shared_guild_rank_management.rs'
target = ROOT / 'apps/gateway/tests/shared_guild_rank_terminal.rs'
assert not target.exists()
text = source.read_text(encoding='utf-8')
prefix = text[:text.index('#[test]')]
tests = r'''
// Ordinary typed packets through the production wrapper; no injected success.
#[test]
fn guild_rank_actor_receives_original_status7_after_canonical_refresh() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("terminal-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("terminal-b"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("terminal-b"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 2);
    login(&mut outsider, "outsider", 3);
    let before = persisted(&config);
    let reply = send(&mut leader, edit(2, "Veterans"));
    assert_only_rename(&before, &persisted(&config), 2, "Veterans");
    let terminals = reply.iter().filter(|p| matches!(p,
        ServerPacket::GuildMemberChange { status: 7, .. })).collect::<Vec<_>>();
    assert_eq!(terminals.len(), 1, "full projection is not the source terminal: {reply:?}");
    assert!(matches!(terminals[0], ServerPacket::GuildMemberChange {
        name, rank_index: 0, status: 7, ranks
    } if name == "Leader" && ranks.len() == 1 && ranks[0].index == 2
        && ranks[0].name == "Veterans" && ranks[0].options == 0
        && ranks[0].members.len() == 1 && ranks[0].members[0].name == "Member"));
    assert!(source_rank_event(&member.execute(WorldCommand::Tick).unwrap(), 2, "Veterans"));
    assert!(!has_rank(&outsider.execute(WorldCommand::Tick).unwrap(), 2, "Veterans"));
}

#[test]
fn guild_rank_repeated_same_name_has_source_terminal_for_each_valid_request() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("terminal-repeat"));
    login(&mut leader, "leader", 0);
    let before = persisted(&config);
    for offset in 1..=2 {
        let reply = send(&mut leader, edit(2, "Members"));
        assert_eq!(persisted(&config).revision, before.revision + offset);
        assert!(source_rank_event(&reply, 2, "Members"), "same name is still a source reply: {reply:?}");
    }
}

#[test]
fn guild_rank_denied_request_has_no_success_terminal_or_durable_change() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("terminal-denied"));
    login(&mut member, "member", 2);
    let before = persisted(&config);
    let reply = send(&mut member, edit(2, "Forbidden"));
    assert_eq!(persisted(&config), before);
    assert!(!reply.iter().any(|p| matches!(p, ServerPacket::GuildMemberChange { status: 7, .. })));
}
'''
target.write_text(prefix + tests, encoding='utf-8', newline='\n')
record = {'fixture_source': source.as_posix(),
    'fixture_source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
    'fixture_prefix_sha256': hashlib.sha256(prefix.encode()).hexdigest(),
    'target': target.as_posix(), 'target_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
    'authority_old_fixture_not_edited': True}
(OUT / 'gateway-terminal-fixture-origin.json').write_text(json.dumps(record, indent=2)+'\n', encoding='utf-8')
print(json.dumps({'created': target.as_posix(), 'sha256': record['target_sha256']}))

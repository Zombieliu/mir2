from pathlib import Path
import hashlib

root = Path(__file__).resolve().parents[2]
work = Path(__file__).resolve().parent
current = (root / 'mir2-web3/apps/simulation/src/runtime/exact_drop_uid_tests.rs').read_bytes()
text = current.decode('utf-8')
text = text.replace('const DROP_DESCRIPTION: &str = "drop uid test";\n\n', '')
text = text.replace('&template.name, DROP_DESCRIPTION, 8,', '&template.name, "drop uid test", 8,')
text = text.replace('    let mut base = embedded_item_state_from_template(&template, container, slot);\n    base.description = DROP_DESCRIPTION.into();',
                    '    let base = embedded_item_state_from_template(&template, container, slot);')
begin = text.index('    // Prove the first unit really can merge before the second lacks a cell.')
end = text.index('    assert!(!preview(&session, &source));', begin)
text = text[:begin] + text[end:]
proof = '''    assert!(item_stack_identity_compatible(&existing,
        &carried(&source, ItemContainer::Bag1, 8, source.item.unique_id, 1)));
'''
assert text.count(proof) == 2
text = text.replace(proof, '')
begin = text.index('    let template = template_for(&source);', text.index('fn fresh_merge_only_is_fenced'))
end = text.index('    for policy in [ItemUidIssuance::Unavailable, ItemUidIssuance::Fenced]', begin)
text = text[:begin] + text[end:]
data = text.encode('utf-8')
# Source06 mixed CRLF/LF source bytes are already present in the current source;
# transformations remove only Root's LF patch. Verify before preserving evidence.
expected = 'dee6fb6e50e367bf988065798d4f21b3ed2d203afeacf8d4fbeb9132af77d1c4'
assert hashlib.sha256(data).hexdigest() == expected, 'Source06 inverse does not reproduce exact tested bytes'
assert len(data) == 17551
out = work / 'failed-source06-exact-drop-uid-tests.rs'
assert not out.exists(), 'Preserve the existing historical source'
out.write_bytes(data)
print('Exact Source06 failed fixture preserved; SHA256 and length verified.')

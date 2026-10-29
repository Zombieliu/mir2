"""Exercise the actual pinned Wallet28 archive path parser; no build or publish.

This is a source-level backwards-admission check, not an old binary migration
or signature/installation result. Git history supplies the immutable old parser.
"""
import hashlib
import subprocess
import sys
import types
import unittest
from pathlib import Path

REVISION = '33f0b8c252f65cc4a24b133292cd1f9eba122dae'
OLD_PATH = 'mir2-web3/apps/game-client/windows-updater/scripts/archive-update-release.py'
OLD_SHA256 = 'd219f3ee29c598a759165ba5b264dfd2cb950067b25f377fb09fae6a945dc61c'
DOCUMENTS = ('README.zh-CN.txt', 'BILLING-HAN-SOURCE.json',
             'OFL-NotoSansSC-BillingHan.txt', 'SIMPLIFIED-SOURCE.json',
             'OFL-NotoSansCJKsc.txt')


class OldRoot12Compatibility(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        raw = subprocess.check_output(['git', 'show', REVISION + ':' + OLD_PATH], cwd=Path(__file__).parent)
        if hashlib.sha256(raw).hexdigest() != OLD_SHA256:
            raise AssertionError('pinned old ROOT12 source changed')
        module = types.ModuleType('mir2_pinned_old_root12')
        module.__file__ = str(Path(__file__).with_name('archive-update-release.py'))
        sys.modules[module.__name__] = module
        exec(compile(raw, OLD_PATH, 'exec'), module.__dict__)
        cls.old = module

    @classmethod
    def tearDownClass(cls):
        sys.modules.pop(cls.old.__name__, None)

    def test_pinned_old_parser_is_root12_and_rejects_r10_root_documents(self):
        self.assertEqual(len(self.old.GAME_ROOT_FILES), 12)
        for name in DOCUMENTS:
            with self.subTest(name=name), self.assertRaises(self.old.ValidationError):
                self.old.game_path(name)

    def test_each_new_legal_path_passes_the_unchanged_old_parser(self):
        for name in DOCUMENTS:
            with self.subTest(name=name):
                self.old.game_path('mir2-assets/legal/' + name)

    def test_old_traversal_and_executable_rejection_is_unchanged(self):
        for path in ['mir2-assets/legal/../source.json', 'mir2-assets/legal/payload.exe.png',
                     'mir2-assets/legal/OFL.txt.cmd', 'mir2-assets/legal/source.json:stream',
                     'mir2-assets/legal/CON.txt', 'mir2-assets/legal/README.txt/../../game.exe']:
            with self.subTest(path=path), self.assertRaises(self.old.ValidationError):
                self.old.game_path(path)


if __name__ == '__main__':
    unittest.main()

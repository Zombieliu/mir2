"""Bounded codec and mocked owner transport only; no SSH or service actions."""
import base64
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import random
import unittest
from unittest.mock import patch
import zlib

HERE=Path(__file__).parent
spec=importlib.util.spec_from_file_location('new_v9_codec_target',HERE/'cloud_test_release_scene_v9.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)


def packed(raw):return base64.b64encode(zlib.compress(raw)).decode('ascii')


def envelope(phase='prepared',size=100000):
    return {'action':'install' if phase=='prepared' else 'apply-caps','proof':{'records':['complete-audit-row']*100},
      'state':{'phase':phase,'predecessor':{'hash':'a'*64},'strictArtifactProof':{'files':['native-file']*88},
        'fullProofRetained':'p'*size},'oldBinaryRollbackAllowed':False}


class BoundedCodec(unittest.TestCase):
    def test_full_large_proof_roundtrips_with_no_dropped_fields(self):
        value=envelope();encoded=m.encode_remote_payload(value)
        self.assertGreater(len(json.dumps(value).encode())*4/3,98304)
        self.assertEqual(m.decode_remote_payload(encoded),value)
        self.assertLessEqual(len(encoded),98304)

    def test_all_phase_envelopes_preserve_full_proofs(self):
        for phase in ['prepared','installed','caps-applied','recovery-required']:
            with self.subTest(phase=phase):
                value=envelope(phase);self.assertEqual(m.decode_remote_payload(m.encode_remote_payload(value)),value)

    def test_raw_decompressed_bound_is_enforced_both_directions(self):
        value=envelope(size=140000)
        with self.assertRaisesRegex(m.Refusal,'remote_payload_json_unbounded'):m.encode_remote_payload(value)
        with self.assertRaisesRegex(m.Refusal,'remote_payload_json_unbounded'):m.decode_remote_payload(packed(json.dumps(value).encode()))

    def test_wire_limit_is_unchanged_and_encoder_also_enforces_it(self):
        self.assertEqual(m.MAX_REMOTE_PAYLOAD_ENCODED,98304);self.assertEqual(m.MAX_REMOTE_PAYLOAD_JSON,131072)
        with patch.object(m.zlib,'decompressobj') as called,self.assertRaisesRegex(m.Refusal,'remote_payload_unbounded'):
            m.decode_remote_payload('A'*98308)
        called.assert_not_called()
        value={'incompressible':''.join(random.Random(194).choices('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789',k=120000))}
        self.assertLess(len(json.dumps(value,separators=(',',':')).encode()),131072)
        with self.assertRaisesRegex(m.Refusal,'remote_payload_unbounded'):m.encode_remote_payload(value)

    def test_invalid_base64_and_uncompressed_legacy_payload_refuse(self):
        for value in ['not-base64!','A','é',base64.b64encode(b'{"action":"install"}').decode()]:
            with self.subTest(value=value[:16]),self.assertRaises(m.Refusal):m.decode_remote_payload(value)

    def test_zlib_bomb_is_cut_off_at_one_byte_over_bound(self):
        encoded=packed(b'{"proof":"'+b'x'*(4*1024*1024)+b'"}')
        with self.assertRaisesRegex(m.Refusal,'remote_payload_json_unbounded'):m.decode_remote_payload(encoded)

    def test_truncated_stream_and_trailing_or_concatenated_data_refuse(self):
        raw=zlib.compress(b'{"valid":true}')
        for candidate in [raw[:-1],raw[:-6],raw+b'garbage',raw+zlib.compress(b'{}')]:
            with self.subTest(size=len(candidate)),self.assertRaisesRegex(m.Refusal,'remote_payload_zlib_stream_not_exact'):
                m.decode_remote_payload(base64.b64encode(candidate).decode())

    def test_invalid_utf8_json_non_dict_and_non_json_constants_refuse(self):
        for raw in [b'{"x":"\xff"}',b'{broken',b'[]',b'null',b'1',b'{"n":NaN}',b'{"n":Infinity}']:
            with self.subTest(raw=raw[:14]),self.assertRaises(m.Refusal):m.decode_remote_payload(packed(raw))
        for value in [[],None,{'notFinite':float('nan')}]:
            with self.assertRaises(m.Refusal):m.encode_remote_payload(value)

    def test_empty_wrong_types_and_invalid_compression_refuse(self):
        for value in ['',None,b'bytes',1,base64.b64encode(b'bad-zlib').decode()]:
            with self.subTest(value=repr(value)),self.assertRaises(m.Refusal):m.decode_remote_payload(value)

    def test_remote_restores_deduplicated_immutable_state_before_transaction(self):
        value=envelope(size=50);value['predecessor']={'wrong':True};value['strictArtifactProof']={'wrong':True}
        called=[]
        def transaction(action,host,config,state):
            self.assertEqual(config['predecessor'],state['predecessor'])
            self.assertEqual(config['strictArtifactProof'],state['strictArtifactProof'])
            self.assertEqual(state,value['state']);called.append(action);return {'passed':True}
        output=io.StringIO()
        with patch.object(m.os,'geteuid',return_value=0,create=True),patch.object(m.signal,'SIGHUP',1,create=True),patch.object(m.signal,'signal'),\
          patch.object(m,'SceneNativeHost',return_value=object()),patch.object(m,'transaction',side_effect=transaction),contextlib.redirect_stdout(output):
            m.remote_main(m.encode_remote_payload(value))
        self.assertEqual(called,['install']);self.assertEqual(json.loads(output.getvalue().splitlines()[0]),{'rpcStarted':True})

    def test_invalid_remote_input_never_announces_rpc_or_calls_host(self):
        output=io.StringIO()
        with patch.object(m,'SceneNativeHost') as host,patch.object(m,'transaction') as transaction,contextlib.redirect_stdout(output):
            with self.assertRaises(m.Refusal):m.remote_main(packed(b'[]'))
        self.assertEqual(output.getvalue(),'');host.assert_not_called();transaction.assert_not_called()

    def test_real_ssh_call_mock_carries_the_exact_new_codec_and_retains_dedup(self):
        # Reuse only a fake channel, never an actual SSH connection.
        fixture_spec=importlib.util.spec_from_file_location('wire_v9_fixtures',HERE/'test_cloud_test_release_scene_v6_v9_retargeted.py')
        fixture=importlib.util.module_from_spec(fixture_spec);fixture_spec.loader.exec_module(fixture)
        value=envelope(size=100);state=value['state']
        config={k:v for k,v in value.items() if k not in ['action','state']}
        config.update(predecessor=state['predecessor'],strictArtifactProof=state['strictArtifactProof'])
        import shlex
        from types import SimpleNamespace
        class Client:
            def exec_command(self,command,timeout):
                argv=shlex.split(command);assert argv[:3]==['sudo','-n','python3']
                decoded=m.decode_remote_payload(argv[-1])
                assert decoded=={**{k:v for k,v in config.items() if k not in ['predecessor','strictArtifactProof']},'action':'status','state':state}
                channel=fixture.legacy.FakeSSHChannel('nopasswd',b'never','synthetic','ignored','status')
                return (SimpleNamespace(channel=channel),)*3
        self.assertTrue(m.ssh_call(Client(),None,'status',config,state)['passed'])


if __name__=='__main__':unittest.main(verbosity=2)

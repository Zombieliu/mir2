"""Additional fixed V8 proof negatives; synthetic public data, no SSH/game."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

HERE=Path(__file__).parent
spec=importlib.util.spec_from_file_location('v8_actual_target_fixtures',HERE/'test_cloud_test_release_scene_v6_retargeted.py')
f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)
m=f.m


class Fixed7403Proof(unittest.TestCase):
    def reject(self,mutate):
        value=f.artifact_proof();mutate(value)
        with self.assertRaises(m.Refusal):m.validate_artifact_proof(value)
        host=f.FakeHost();host.config['protectedProof']=host.proof();host.config['strictArtifactProof']=value
        result=m.transaction('prepare',host,host.config)
        self.assertFalse(result['passed']);self.assertEqual(host.actions,[])

    def test_exact_new_source_contract_accepts_synthetic_proof_only(self):
        value=f.artifact_proof()
        self.assertIs(m.validate_artifact_proof(value),value)
        self.assertEqual(m.CANDIDATE_REVISION,'7403bc5ee5cbb472477db101b8be603136049dcf')
        self.assertEqual(m.INTEGRATION_PARENT,'38a911769a5ec2d012c538dcb95f454ef1ca37c9')
        self.assertEqual((len(m.COMMIT_DELTA_PATHS),len(m.SOURCE_PATHS),len(m.NATIVE_BUILD_INPUT_OBJECTS)),(10,88,11))

    def test_old_v7_candidate_schema_and_verifier_are_not_accepted(self):
        for field,value in [('schema','mir2.scene-candidate-artifact-verification.v7'),
            ('candidateRevision','5a356ae0d5b79b846675f623bd5d2dc21c09a10f'),
            ('integrationCommit','6f04b710766a9e7a2deb553e0f5ec0e1761db598'),
            ('verifierSourceSha256',m.V7_VERIFIER_SHA256)]:
            with self.subTest(field=field):self.reject(lambda p,k=field,v=value:p.update({k:v}))

    def test_whole_workflow_and_exact_native_job_remain_required(self):
        for field,value in [('wholeWorkflowStatus','in_progress'),('wholeWorkflowConclusion',None),
            ('wholeWorkflowConclusion','failure'),('runId',43432291642),('nativeJobId',131908522957),
            ('nativeRequiredGates',m.REQUIRED_GATES[:-1]),('nativeStartupActualExitCode',False)]:
            with self.subTest(field=field,value=value):self.reject(lambda p,k=field,v=value:p.update({k:v}))

    def test_exact_parent_commit_and_no_unreviewed_tenth_file(self):
        for field,value in [('integrationParent','0'*40),('sourceNormalization','crlf-to-lf'),
            ('protectedWorkflowAndPackagingScriptIdentical',False),('exactIntegrationCommitDelta',m.COMMIT_DELTA_PATHS[:-1]),
            ('exactIntegrationCommitDelta',m.COMMIT_DELTA_PATHS[:-1]+['mir2-web3/apps/gateway/src/unreviewed.rs'])]:
            with self.subTest(field=field):self.reject(lambda p,k=field,v=value:p['sourceCompatibility'].update({k:v}))

    def test_all_eighty_eight_rows_exact_and_order_hash_required(self):
        variants=[m.SOURCE_PATHS[:-1],m.SOURCE_PATHS+['mir2-web3/apps/gateway/src/extra.rs'],
            m.SOURCE_PATHS[:-1]+[m.SOURCE_PATHS[0]],list(reversed(m.SOURCE_PATHS))]
        for paths in variants:
            with self.subTest(paths=len(paths)):self.reject(lambda p,v=paths:p['sourceCompatibility'].update(reviewedSourcePaths=v))

    def test_all_eleven_objects_are_fixed_not_merely_any_sha(self):
        for change in ['drop','sha','path','duplicate','order']:
            def mutate(p):
                rows=p['sourceCompatibility']['nativeInputIdentity']['nativeBuildInputObjects']
                if change=='drop':rows.pop()
                elif change=='sha':rows[0]['gitObject']='0'*40
                elif change=='path':rows[0]['path']='unreviewed'
                elif change=='duplicate':rows[-1]=copy.deepcopy(rows[0])
                else:rows.reverse()
            with self.subTest(change=change):self.reject(mutate)

    def test_old_non_native_delta_is_not_a_substitute_for_new_identity(self):
        self.reject(lambda p:p['sourceCompatibility'].update(canonicalNonNativeDelta={'path':'.github/scripts/numeron-r16-release.mjs'}))
        for key,value in [('candidateParent','0'*40),('candidateSourceIsIntegrationCommit',False),
            ('exactWholeRepositoryCommitDelta',m.COMMIT_DELTA_PATHS[:-1]),('nonNativeDeltaClaimed',True),
            ('nativeJobBodySha256','0'*64)]:
            with self.subTest(key=key):self.reject(lambda p,k=key,v=value:p['sourceCompatibility']['nativeInputIdentity'].update({k:v}))

    def test_actual_binary_artifact_ids_hashes_and_manifest_are_fixed(self):
        for field,value in [('binarySha256','0'*64),('binarySizeBytes',m.CANDIDATE_BINARY_BYTES-1),
            ('releaseManifestSha256','0'*64),('sourceInputManifestSha256','0'*64)]:
            with self.subTest(field=field):self.reject(lambda p,k=field,v=value:p.update({k:v}))
        for role in ['gateway','nativeEvidence']:
            for key,value in [('id',1),('sha256','0'*64)]:
                with self.subTest(role=role,key=key):self.reject(lambda p,r=role,k=key,v=value:p['artifacts'][r].update({k:v}))

    def test_c_copy_refuses_source_guard_before_any_remote_connection(self):
        frozen_copy=Path(tempfile.gettempdir())/'mir2-offline-frozen'/'cloud_test_release_scene_v8.py'
        with patch.object(m,'__file__',str(frozen_copy)),patch.object(m,'connect_pinned') as connected,self.assertRaisesRegex(m.Refusal,'canonical_operator_source_required'):
            m.source_guard()
        connected.assert_not_called()

    def test_canonical_source_guard_keeps_old_pins_and_adds_only_new_relative_pin(self):
        repository=HERE.parents[2]
        canonical=repository/'scripts/capacity/cloud-reference/cloud_test_release_scene_v8.py'
        real_hash=m.file_hash
        expected=repository/'scripts/capacity/comparable-reference/cloud/verify_scene_candidate_artifact_7403.py'
        def fixed_hash(path):return m.V8_VERIFIER_SHA256 if Path(path)==expected else real_hash(path)
        with patch.object(m,'__file__',str(canonical)),patch.object(m,'file_hash',side_effect=fixed_hash) as hashed,\
            patch.object(m.subprocess,'run',return_value=SimpleNamespace(returncode=0,stdout=b'{}')) as run:
            m.source_guard()
            self.assertIn(((expected,),),[(c.args,) for c in hashed.call_args_list])
            self.assertEqual(run.call_args.args[0],[m.sys.executable,str(repository/'scripts/verify_unified_codeline.py')])
        with patch.object(m,'__file__',str(canonical)),patch.object(m,'file_hash',side_effect=lambda p:'0'*64 if Path(p)==expected else real_hash(p)),\
            patch.object(m.subprocess,'run') as run,self.assertRaisesRegex(m.Refusal,'frozen_v8_fixed_7403_verifier_source_guard_changed'):
            m.source_guard()
        run.assert_not_called()

    def test_same_remote_paths_and_all_nonproof_definitions_are_unchanged(self):
        def definitions(path):return {n.name:ast.dump(n) for n in ast.parse(path.read_text()).body if isinstance(n,(ast.FunctionDef,ast.ClassDef))}
        old,new=definitions(HERE/'cloud_test_release_scene_v7.py'),definitions(Path(m.__file__))
        self.assertEqual(sorted(k for k in old if old[k]!=new[k]),['source_guard','validate_artifact_proof'])
        self.assertEqual(m.OWN_PATHS,[m.RELEASE_DROPIN,m.RELEASE_ENV,m.CAPS_DROPIN,m.CAPS_ENV])
        self.assertEqual(m.UNIT,'mir2-playtest.service');self.assertEqual(m.RELEASE_SUFFIX,'-spectator-scene-20261113-v6')
        self.assertEqual(m.ARTIFACT_SCHEMA,'mir2.scene-candidate-artifact-verification.v8')
        # Loading the legacy core suite must not replace this full wrapper.
        core_spec=importlib.util.spec_from_file_location('isolated_v8_core_boundary',HERE/'test_cloud_test_release_v5_retargeted.py')
        core=importlib.util.module_from_spec(core_spec);core_spec.loader.exec_module(core)
        self.assertIs(core.m.transaction,core.m._transaction_core)
        self.assertIs(core.m.validate,core.m._validate_v5)
        self.assertIsNot(core.m,m)
        self.assertIsNot(m.transaction,m._transaction_core)
        self.assertIsNot(m.validate,m._validate_v5)
        self.assertEqual(Path(core.m.transaction.__code__.co_filename).resolve(),Path(m.__file__).resolve())


if __name__=='__main__':unittest.main(verbosity=2)

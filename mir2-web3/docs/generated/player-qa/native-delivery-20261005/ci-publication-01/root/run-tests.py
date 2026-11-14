from pathlib import Path
import datetime, hashlib, importlib.util, json, os, sys, time, unittest

sys.dont_write_bytecode=True
BASE=Path(__file__).resolve().parent
ROOT=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
path=ROOT/'scripts/test_native_r17_r2_delivery.py'
spec=importlib.util.spec_from_file_location('root_delivery_tests',path)
module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module
spec.loader.exec_module(module)
module.FIXTURES=BASE/'fixtures'
module.FIXTURES.mkdir()
started=time.monotonic()
with (BASE/'ROOT-TESTS-01.log').open('x',encoding='utf-8',newline='\n') as log:
    suite=unittest.defaultTestLoader.loadTestsFromTestCase(module.DeliveryTests)
    result=unittest.TextTestRunner(stream=log,verbosity=2).run(suite)
failed_methods={getattr(test,'test_case',test).id() for test,_ in [*result.failures,*result.errors]}
receipt={'schema':'mir2.native-r17-ci-root-tests.v1','passed':result.wasSuccessful(),
         'testsRun':result.testsRun,'passedMethods':result.testsRun-len(failed_methods)-len(result.skipped),
         'failedMethods':len(failed_methods),'failureAssertions':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),
         'sourceSha256':module.P.source_sha(),'testSha256':hashlib.sha256(path.read_bytes()).hexdigest(),
         'planSha256':hashlib.sha256(module.PLAN_RAW).hexdigest(),'actualNetworkRequests':0,
         'opaqueCredentialsRead':False,'remoteR2Proved':False,'python':sys.version,'platform':sys.platform,
         'elapsedSeconds':time.monotonic()-started,'createdUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
with (BASE/'ROOT-TESTS-RECEIPT-01.json').open('x',encoding='utf-8',newline='\n') as out:json.dump(receipt,out,indent=2);out.write('\n')
print(json.dumps(receipt))
sys.exit(0 if result.wasSuccessful() else 1)

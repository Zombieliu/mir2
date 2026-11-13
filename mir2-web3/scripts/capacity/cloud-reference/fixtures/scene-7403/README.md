# Offline fixed-7403 controller tests

`source-compatibility.json` is public immutable Git-source evidence used by the
synthetic artifact-proof fixtures. It contains no account or operator credentials.
It is resolved relative to the test files, independently of a temporary C bundle.

From the repository root, ordinary unittest discovery needs no aliasing runner:

```powershell
python -B -m unittest discover -s mir2-web3/scripts/capacity/cloud-reference -p '*_retargeted.py' -v
python -B -m unittest discover -s mir2-web3/scripts/capacity/cloud-reference -p 'test_cloud_test_release_scene_v8.py' -v
```

These commands run 97 historical tests plus 11 new proof tests. All target
`cloud_test_release_scene_v8.py`. The legacy 54-test file creates its own private
module instance and maps only that instance's public test aliases to the actual
`_transaction_core` and `_validate_v5` functions. Other suites import independent
instances and exercise the full guarded transaction wrapper. No source is copied
or rewritten at test time, and the original V5 test file is retained unchanged.

The C-location refusal test explicitly mocks a noncanonical source location;
running the tests from the canonical tree does not imply that the real source
guard should refuse that tree. The full-wrapper tests verify that importing the
legacy suite cannot replace their transaction or validation functions.

Tests use synthetic hosts/channels and retained C-only ACL fixtures. They perform
no real SSH, service control, game workload, account access or deployment.

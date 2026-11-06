# Windows packed release live evidence

Individual historical/offline/live receipts keep their own scope. No overall acceptance, live migration, player throughput or five-minute SLA is inferred by this index.

| Evidence | State | SHA256 |
| --- | --- | --- |
| [source-offline-index](files/evidence-staging/SANITIZED-SOURCE-AND-OFFLINE-VERIFICATION.json) | available | 94D4588CE2C8C77D8CF57EE0B3FE008792439F3EA1A5B8B749665E6B8BEC6936 |
| [native-build](files/native-release-29a/BUILD-PACKED-NATIVE.json) | available | CEC59E2EB68576416732011B12F75FCDCB04F825A539C0CFF0E92817B9364FF8 |
| [engine-build](files/updater-release-ba79/BUILD-UPDATER.json) | available | 248E3122858FA8DA08CDDDACFFF49DF97E15F7721D908AFE07381DED93CA8018 |
| [bridge29-cms](files/verify-bridge29-bundle.json) | available | 0DF7A7B812950C94E29951D1569C8FD19246A4343E09CDEAF609921A1B28E0E5 |
| [bridge30-cms](files/verify-bridge30-bundle.json) | available | 361BF45C90276AB0309B59EDA137D5B8868D0A8225DC5B5A55557EBEA245A8AB |
| [sign-generation](files/packed-s31/SIGN-PACKED-GENERATION.json) | available | C5439BC261F698D63848B953FB7AAF1A0714A4F18FBE4BB0FE46745E37298122 |
| [verify-generation](files/packed-s31/VERIFY-PACKED-GENERATION.json) | available | 18F84F63C6A0BD344F17F61EBEE112F3F5F454F78253901CE333B902197EE9AF |
| [verify-signed-pack-bytes](files/packed-s31/VERIFY-PACKED-GENERATION.json.packs.json) | available | 5C49E01A72DB0C31CD311BE652295BCB1AC48EC6B857EF355AE098D6E6D9EB33 |
| [resource-source-audit](files/full-resources/SOURCE-AUDIT.json) | available | FFBA62CE45EBDEEAFAD28528057A32B67704A753653DF2908C6C0C1738BD9B7E |
| [full-resource-build](files/full-resources/FULL-RESOURCE-PACK-RESULT.json) | available | 86533D63A24B02A0A41EEB40337ED00D5A5E20105DA847B962DCE96108930420 |
| [full-resource-read](files/full-resources/ACTUAL-RESOURCE-VERIFICATION.json) | available | F3049D7D1CDCF169A28DA5C40C5C07650E61B4731E2BCCAA6515F099B63EF6DD |
| [real-offline-health](files/offline-game-preflight/HEALTH-RECEIPT.json) | available | E71DC7CD0B50DDFC5EAAE43E484166744F4E82E4303BA4B006A2137ACB9E98CD |
| [real-game-exit](files/offline-game-preflight/CHILD-EXIT-RECEIPT.json) | available | 199A15B87E32EAD14200D0C6453073FE61382D3030D0E6399C54B3FDCFDB819D |
| [accepted-fixture-public-noop](files/OFFLINE-HEALTHY-PUBLIC-ORIGIN31-NOOP.json) | no-op-network-function-only | 71D74D48728DE5007B454A5AC8C680653C22C327B88E6F5ABEF86EF384B46386 |
| [accepted-fixture-public-noop-login-ru](files/operations/warm-public-validation-01/login-ru.jpg) | no-op-network-function-only | CFFBC5E4562740D21308731D468DB42CCCAFBE2BA671A0312F84EAF3E0978B5C |
| [origin-http137](files/operations/origin-http-acceptance31.json) | available | 2817A462B4C14F5E6A4575279212EC863654CF559D36869F4FF259BC608B91BD |
| [final-delivery-state](files/operations/FINAL-DELIVERY-STATE-v2.json) | available | AFD79E148AF54B1FFD67E1D90A5C9530B236E59FAD5E57628AA97174338E025F |
| [v4-linux9](files/operations/ssh-results/v4-refresh-extract-linux-tests-01.json) | available | 0ED40519B1ED8536B8FB141AD25C64ABF84897184D41AA7ED806D7BE03F0BAB6 |
| [packed31-r2-stage](files/publisher/stage-real-packed31-remote-v3.json) | reported-not-passed | 40167EA5937660FB7BCC0CC7838FA4D4EBDCF608CDA087C2231D04621A4CBDCA |
| [packed31-prefill-failed](files/publisher/prefill-real-packed31-parallel-v1.json) | reported-not-passed | BA7446B853A0B2CA493660594F2A88A23B3E3C2D0BC308B200F0D9E81C0F43C2 |
| [packed31-r2-stage-persistent](files/publisher/stage-real-packed31-persistent-v4.json) | reported-not-passed | 978D01AA58F6289D56D44FFC96186EF5EC8D6AC18DEF6741AA574A022B591790 |
| [five30-r2-stage](files/publisher/stage-real-five30-v2.json) | available | 0027A9E6827C29D8D1278F5458E53CB7D326CAAD10C52C1A372B30D9230ED8EB |
| [legacy29-r2-stage](files/publisher/stage-real-legacy29-v2.json) | available | FC97F8F7F315BB3F947F237BE0293D37F260CD47B353AF8BFE8E995CE0C9FC5E |
| packed31-r2-promote | superseded | pending |
| packed31-r2-promote-persistent | blocked-by-stage | pending |
| five30-r2-promote | held-for-performance | pending |
| legacy29-r2-promote | held-for-performance | pending |
| assets-http137-packed31 | blocked-by-stage | pending |
| assets-http137-final | blocked-by-rollout | pending |
| [bounded-network-diag](files/network-diagnostic/BOUNDED-CATALOG-RANGE-DIAGNOSTIC.json) | available | 755E12C01DBB03A737AC8109A51336B0DDE92E64ACD24898F3C6CE1E37282E36 |
| [network-prefix-binding](files/network-diagnostic/PREFIX-COMPARISON.json) | available | 2B6E0E1E1062E053847FB7735BC8588637C5A4D9569EE2B198398CE6FF70CE2E |
| [updater-proxy-audit](files/network-diagnostic/UPDATER-PROXY-FEATURE-AUDIT.json) | available | 288930A1498DF0340B67D37B58CA155223B2A648ACC9BED7EEBA201C5B401478 |
| cold-migration-observer | blocked-by-rollout | pending |
| [publisher-cleanup](files/CLOUDFLARE-PUBLISHER-CLEANUP.json) | available | 755D1937E3A16E9B51C0497424E20CD7D7CE395D2C378E10C84FCE693FAB407B |
| [backend-final](files/operations/ssh-results/origin-final-services-01.json) | available | E077BE96AABDDBC8F71AD769A30781797A3089DC5AC27581606B16CF00B9834F |
| [rollout-decision](files/ROLLOUT-DECISION.json) | available | BEBB167FFA748252BE71D0BB466AF90115EF53A1670BEC05B3DF6D685FFEC55A |
| [baseline-login-ru](files/baseline-language-ru.jpg) | available | CFDAA9C0F91FB2ABAC5ED5C491AA0F4DE3C5A62E2AF58314019A6C47B7A0C169 |
| [packed-login-ru](files/packed-language-ru.jpg) | available | 2AD34DE99265430E9F418AE76A1C103F2BDB07A3C210AAFFB330FB3D82796B56 |

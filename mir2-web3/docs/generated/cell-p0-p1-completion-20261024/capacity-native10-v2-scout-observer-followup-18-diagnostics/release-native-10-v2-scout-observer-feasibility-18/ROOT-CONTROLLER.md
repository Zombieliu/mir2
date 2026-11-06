# Root18 seal and readonly sourceguard

These are source-control modes only. No provisioning,accounts,binary build,realm or collection is invoked. Backend/build is c743122af05faca43d9535d8c6c2a4bfb01b924f; current execution/driver is 3a6d8c862d03ded3133605472a8f38bc6258af52; Node219 and the original c74350/dae1 Release remain separate. Original2/10×300+360/one fighter/5-5-6 windows/caps24-10-10-4-1-2/resources are unchanged; capacityAccepted is always false.

Seal requires an actual-bound runtime-prepared revision and the exact final independent review. It checks every pinned source,checks every static prepare reference,runs real Node--check/PowerShell AST checks,writes launcher-syntax-check.log exclusively and includes that exact log in the seal. Sourceguard is a separate readonly mode after sealing. Real seeding remains a later Root operation. Private owner1 after seeding and recordings owner+SYSTEM2 checks remain in the unchanged prepare code.

After final review,Root supplies the exact final revision and its raw hashes:

    node root-seal-and-guard-v1.mjs --seal-reviewed-inputs --prepared-file=<absolute-runtime-prepared-v3.json> --prepared-sha256=<actual64hex> --review-file=<absolute-finalreview.json> --review-sha256=<actual64hex>
    node root-seal-and-guard-v1.mjs --readonly-sourceguard
    node prepare-inputs.mjs --prepare-authorized-inputs
    node run-owned.mjs --execute-authorized-probe --attack-results=v2

The current author has executed only Node syntax checks; no above mode was run. Initial controller construction failed before source output because of an LF/CRLF pattern; the resulting missing-target syntax log is retained. The later actual Node--check succeeded. No failure is rewritten as a pass.

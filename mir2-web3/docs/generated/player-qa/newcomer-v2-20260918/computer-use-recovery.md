# Windows control capability recovery

The documented node_repl initialization of @oai/sky and list_windows now
succeeds. No Mir client window is currently present. The previous missing
Trusted RPC checkpoint in runtime.md is retained as historical evidence;
it no longer blocks attempting Native visual QA after the fresh EXE build.
No screenshot or input action was taken by this lightweight capability check.
Other application names and process paths are omitted from exported evidence.

The clean C: Native checkout is pinned to 6e1778957299367b3fbd39961f827c1bcb2d628a.
The attested builder is currently compiling from that source, with one Cargo
job and offline cached dependencies. Its first wrapper attempt had an empty
RUSTC variable because the wrapper used SetEnvironmentVariable(null); it
failed before compiler execution. That wrapper-only error was corrected by
removing Env: entries, and all first-attempt files were preserved separately.
No repository compiler, attestation or publication gate was weakened.

CurrentUser contains no enumerated private-key Code Signing certificate;
formal signed packaging remains a separate prerequisite. A build attestation,
a development launch, protocol completion and visual acceptance are distinct.

visualAccepted=false; signedPackage=false; ordinaryComplete=false.

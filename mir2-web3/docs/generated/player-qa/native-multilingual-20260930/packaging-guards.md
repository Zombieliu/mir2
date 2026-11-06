# Multilingual package guard and installer name follow-up

The first clean release from `2af40352307184b033516e10d200e860ed3bbf82`
built successfully, but its signed Candidate failed the final path-leak guard.
That failed package was not delivered. The retained verification receipt is
`first-package-verification.json`; `detachedSignatureValid=true` does not
override its failed verification result.

The first match, EXE offset 65,644,009, lies inside the numeric `gvar` table of
the embedded Noto Sans TC font, not a source path. The verifier now requires
the pinned 11,941,968-byte font with SHA256
`864727D210D54F2537BBE23B3A839436C3992AF72DE9322AF5270897246BD44F`,
then matches the entire embedded blob byte for byte. Only its fixed `gvar`
range (font offset 7,140,372, length 4,718,420) is cleared in the private scan
copy. Font strings and every surrounding byte remain scanned. The executable
on disk is unchanged. Missing or altered fonts receive no exclusion.

The next match, EXE offset 73,298,772, is the Portuguese help text
`padrão:\n• Pacífico`: an ASCII-only boundary treated the final `o:` as a drive.
Only the Unicode drive-prefix boundary now recognizes Unicode letters,
combining marks and numbers. The ASCII, UNC and PDB rules remain intact;
literal `\n` is not exempted because it can start a real directory name.

`verifier-font-selftest.log` records the passing targeted verifier run,
including a scan of the actual first release EXE, unchanged-file checks,
changed-font rejection, and real machine paths before/after the font and
localized text in multiple encodings. A separate read-only review confirmed
the exact font match and 11 Unicode-boundary comparisons, including NFC/NFD
prose and real drive/UNC/PDB paths. No game code changed in this follow-up;
the 1,147 shared tests, 777 host tests and 47 GPU images are not repeated.

The public installer name is now
`Numeron-Legend-of-Rebirth-20260930-r4-Setup.exe`, version `2026.09.30.4`.
Its wizard, shortcuts and uninstall entry use `Numeron - Legend of Rebirth`.
Stable AppId and installation identity allow upgrades from earlier packages.
The recipe and three player guides carry the same branding.

A new clean attested release and strictly verified package are required after
this source commit. Their final hashes and installer result will be recorded
in a separate release handoff. This record does not claim clean-PC installation
or an actual Alienware-device retest. Capacity work remains paused.

# Prepared server-only appearance delivery

Historical preparation snapshot. The authorized rollout and actual public
checks are recorded in [rollout-01](../rollout-01/README.md).

- Clean source: `c6c32381a646dae1067dafdeec57691f606b1779`.
- Actual CI: [37617054260](https://github.com/Zombieliu/mir2/actions/runs/37617054260),
  overall success. Build/security and reused siege jobs pass.
- Gateway artifact11480868639; exact authenticated ZIP SHA256
  `40297bba9a318697f9ada8a4b51ddde88d2769458f623d8c46e2035c9bc9b9d7`.
- Package SHA256 `33f1fa442231b21b0e2b0c0c57810071a94cb28af83b06f24bcede426aac5fcc`.
- Binary 80703496 bytes / SHA256
  `7ffbea9de2998ff5265b9bb212caf9c56c9d8c61de5dbc3fa3235442217818d9`.
- Private server preparation confirms the same binary/source metadata. Reviewed
  operator SHA256 `2e6259a3bd9649d47464019143ebe14ef45096612a92023d5d6d53a8240186e3`;
  manifest SHA256 `cd1be39e45b7d368bd7733e69732b11f161597ed98c50bd18ee10526345c6c9e`.
  Intended request passes; four wrong requests fail. No service change occurs.

Zone appearance5/Gateway3 and atomic restore2 pass. Linux repeats appearance
5/3 and confirms cold-world cleanup1, overlapping prior Windows checks. Full
shared Zone regression is208/1; identical unchanged-source8fa failure remains.
Original RED, compile failures, Windows LNK1104 and incorrect archive-name
verifier failure remain in the evidence; none are called successful.

The two real online actors have not been disconnected. R19 Gateway and native
R20/signed16 remain live. Human normal exit approval is pending; no restart,
database restore, installer rebuild or live appearance acceptance is claimed.
The public acceptance harness is prepared with fresh ordinary accounts, normal
TLS and no debug/admin commands, but has not yet run. No actual user account
credentials, mail recipient identifiers, private saves or binary assets are
included in this archive. The requested50000 gold mail receipt stays private.

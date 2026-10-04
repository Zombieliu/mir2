# Bounded parallel origin download diagnostic

The exact user r16 Bootstrap URL honors verified-TLS direct HTTP1.1 Range.
Only7MiB total was requested, with at most4 concurrent connections; no full
installer download, proxy use or server/service/save change occurred. Actual
current-development-host aggregates are0.11145MiB/s (1 connection,1MiB,8.973s),
0.09857MiB/s (2,2MiB,20.289s), and0.20438MiB/s (4,4MiB,19.571s). The same first
range hash matches across modes. Short windows vary; these do not establish a
constant per-connection limit, a hosting bandwidth plan, or the other laptop's
speed. They are not actual updater or R2/CDN acceptance.

At the observed four-connection aggregate,620MB would still take roughly48
minutes ignoring overhead. The frozen existing updater uses serial artifacts,
so that estimate is not a claim it reaches the parallel probe rate. Signed
bundle publication and real old-Bootstrap use need separate verification;
Cloudflare/R2 deployment remains dependent on real account authentication.
INDEX.json binds the exact script and all raw outcomes.

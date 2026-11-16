# Mail reader source audit — 2026-09-21

Implementation specification, not live acceptance. Source: E:/mir2/Crystal/Client/MirScenes/Dialogs/MailDialogs.cs; native render_mail in client-bevy overlays.rs. Current native Read only sends unread status and never opens a source reader. Native also disables Read for already-opened mail. Source Read button or second row click opens Letter if no attachments/gold, Parcel otherwise, regardless of already-read state; only sends C.ReadMail for !Opened.

| Surface | Exact source geometry |
| --- | --- |
| Letter | Title672, declared236x300, movable at100,100. Prguse2 close360/361/362 at209,3; sender70,35,150x15; date70,56,150x15; body15,92,202x165. Title delete540/541/542 at12,265; lock686/687/688 at81,265; cancel193/194/195 at154,265. |
| Parcel | Title675, declared236x300, movable at100,100. Same close/sender/date. Body15,98,202x165; gold63,290,143x15; five35x31 lime Mail cells at27+36*i,311. Title initial collect370/371/372 at30,350; cancel193/194/195 at135,350. Source child extent reaches375 despite declared300: inspect real source asset dimensions/clipping before implementing. |

Source date dd/MM/yy H:mm:ss; body replaces literal backslash-r/backslash-n with real CRLF. Letter Delete rejects locked, sends and hides; Lock toggles then sends LockMail. Parcel source enables collect680/681/682 when Mail.Collected is true, otherwise disabled683/684/685. Native currently claim-enabled when !claimed; audit wire semantics before changing either. Reply is hidden unless CanReply and prefills sender; native currently renders disabled placeholder.

Protocol ClientMail already has can_reply/date_sent_binary_datetime but platform gateway transform and shared MailMessage drop them. Add metadata through bridge/model with safe date conversion (blank invalid/zero). Attachments have item_index but lack source ItemInfo.Image/complete tooltip; propagate real metadata or authoritative lookup, never fabricate icons. NativeOutboundCommand/intent lacks LockMail though browser gateway supports it.

Asset audit found missing Title frames672,675,540-542,686-688,370-372,680-685,676 from export/meta;193 exists68x25, close Prguse2 frames exist24x21. Title metadata source SHA matches local source library. Inspect/export the actual indexed originals using existing exporter, do not draw substitutes. Source row Prguse540/541/550/551/552 and Title670 layout differ from current textual list adapter.

Next implementation: bounded MailReaderUi keyed by exact mailID, separate Letter/Parcel roots, movable origin, current authoritative record lookup, close on deleted/kind-invalidated record; open already-read without status send. Wire lock before enabling it. Cover open/reopen, data refresh/identity, five attachments and missing metadata, source date, body wrapping, claim/lock/delete guards, Escape/close and input isolation. Test assets and render, then stage matched client and same-build live acceptance after valid desktop handoff. Do not send real messages without specific authorization.

Native status retry locks corrected in5be3a627e; legacy Read/Delete pending variants are no longer allocated by native intents. Collect/Send remain correlated. No live read/collect/delete or source screenshots were taken for this audit.

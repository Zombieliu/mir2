# Current whole-UI acceptance matrix

Goal scope is unchanged. Latest staged feedback package is C:/numeron-legend-of-rebirth-20260921-mail-feedback, client SHA256 1ACF0FFBD7249A3EF7C1AA1406F2B257A93B1C2E6E6FD47CC921940894A4639B (see README for matched Gateway). No same-build native screenshot or desktop operation proves acceptance for this package. Test counts are code evidence, not a page-acceptance denominator. Historical partial screenshots in README.md belong to explicitly older packages.

Every row below requires actual visible rendering, correct inputs/state/close behavior, applicable Crystal source or original-client comparison, and an evidence path tied to the tested executable. Status **open** means not accepted, including rows with implemented code and passing tests.

| Page / flow | Required ordinary-user verification | Current acceptance |
| --- | --- | --- |
| Login | demo login, wrong-password error, reconnect, no duplicate session | Open; older login observation only |
| Registration | Empty form, validation, cancel, public create/error response | Open; older form/cancel only |
| Password | Masking, validation, cancel, authorized change/error response | Open; older cancel only |
| Character selection | Real roster, select, enter, normal return/save | Open; older roster/logout only |
| Character creation | Each class/gender preview, name validation, create/cancel | Open |
| HUD | HP/MP/experience, counters, action bar, menu positioning | Open |
| Chat | Focus/edit/channel, history scroll, close/focus isolation | Open; do not send to others without authorization |
| Bag | Tabs, real icons/counts, empty/swap/merge/split, item menu lifecycle | Open; older empty/swap only |
| Equipment | Equip/remove, correct preview/stats/tooltips, close over minimap | Open; older close only |
| Skills | Warrior/Wizard/Taoist pages, progression, assign/use, icons/animations | Open |
| Shortcut settings | Rebind, duplicate/conflict, persistence, thumb/arrows/reset | Open; older arrow observation only |
| Quest diary | Multiple active quests, selection/details, long labels, completion updates | Open |
| Quest tracking | Main choice, destination/coordinates, map link, target counter/route | Open; older objective card only |
| NPC dialogue | Pages/options/close, no background click, current NPC identity | Open |
| Small map | Terrain/markers/day-night, move/zoom/toggle, no stale resources | Open |
| Large/world map | Search, NPC/hunt areas, map transitions, go-to/cancel path | Open |
| NPC buy | Item list/scroll, quantity/cost, inventory/gold after receipt | Open |
| NPC sell | Drag correct instance/count, quote/confirm/cancel, authoritative balances | Open |
| Repair / special repair | Eligible item, quote, affordability, confirmed durability | Open |
| Storage | Password flows, capacity/rental, pages, drag/swap/merge, persistence | Open |
| Trade | Two ordinary players, items/gold/cancel/confirm/reject, no duplicate assets | Open |
| Mail list/read | Original rows, letter/parcel, read/lock, delete warning, claim | Open; source-backed candidate only |
| Mail compose | NPC parcel entry, recipient, text, attachments, postage/stamp, send/error | Open; generic form remains; original contract gaps documented |
| Auction | Browse/search/pages, actual eligible listing/cancel/purchase receipts | Open |
| Game shop | Categories, currency/prices, pages/details, ordinary purchase/error | Open |
| Friends / blacklist | Empty/populated list, selection, authorized actions, persistence | Open; older empty/tabs only |
| Group | Invitation/accept/decline/leave and member status between ordinary players | Open |
| Guild | Membership gates, notice/rank/storage/gold/skill pages and receipts | Open |
| Settings | Toggles/sliders/persistence and actual render/audio effects | Open |
| Help | All45 source pages, page controls/drag/close/content fit | Open; older first3 dynamic pages only |
| Pets / creatures | Available roster, commands/feed/state and item changes | Open |
| Mount | Eligibility/equip/mount/dismount, preview and movement | Open |
| Fishing | Equipment/bait/start/end/results, input/state cleanup | Open |
| Hero | Eligibility/open/equipment/skills/actions and correct owner/hero targeting | Open |
| Logout / exit | Correct separate actions, confirmation, durable save, roster/login/exit | Open; older normal roster return only |
| Cross-page input | Overlap/topmost, modal closing frame, drag cancellation, focus loss | Open |
| Scaling and stability | Supported window modes/DPI, readable bounds, running/map transitions, soak | Open; old29GB memory blocker unresolved |

Desktop input stopped after detected manual interference. Pending handoff is separate from standing demo/demo credential authorization. Code work can proceed, but no screenshots may be attributed to unlaunched candidates. The memory cause is unproven until attribution/reproduction; stable high usage is not a pass.

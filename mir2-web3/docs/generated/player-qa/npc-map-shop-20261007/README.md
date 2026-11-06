# NPC map/shop Candidate evidence

See [implementation and scope](../../../NATIVE-NPC-MAP-SHOP-20261007.md).

Passed: Bevy map 37, shop 20, Windows quest route 23, Windows map input 9;
explicit GPU 1, including two actual frames. The selected-row and shop frames
are offline production-renderer fixtures, not Windows/public user acceptance.
The 19-item shop catalog is synthetic. Original assets, eight visible original
icons and the Traditional Chinese pickaxe label are checked.

[Selected Smith](selected-smith.png), [shop last page](shop-last-page.png),
[exact GPU scope](scope.json). Raw logs retain all earlier compile/setup/input
fixture failures and the corrected passes. No final human acceptance claim,
new public game/feed, protected install or real save mutation is made here.

Commands use cargo +1.95.0 --locked; client-bevy needs --features native-ui.
Filters: big_map, npc_shop, quest_route_input_tests, big_map_input_tests.
GPU filter big_map_npc_selected_row_offscreen_visual is explicitly run alone
with --ignored and the original asset root. Native route checks explicitly use
MIR2_NATIVE_ASSET_ROOT; an absent owned-worktree collision pack is a failure.

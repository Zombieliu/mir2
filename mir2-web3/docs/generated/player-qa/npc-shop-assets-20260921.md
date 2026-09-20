# NPC shop UI audit and missing purchase assets

The reported Samuel purchase panel displayed goods over the world without its
frame or Buy button. The shared renderer requests Prguse/1000 and Title/312–314,
but all four were absent from the export manifest and local assets.
Crystal Client/MirScenes/Dialogs/NPCDialogs.cs, NPCGoodsDialog constructor,
confirms those indices and libraries.

Added the four original source images, export requirements, per-library metadata
and global manifest entries. Source library hashes match existing metadata.
All 15 purchase-frame/button/new-marker resources decode with visible pixels.
The original frame was inspected: 244x334; Buy button is 80x25. Native layout
currently requests 242x330 and 80x22, so exact pixel geometry remains outstanding.

Scope: all NPCs using the common purchase panel receive the asset repair.
The code implements buy/sell/repair/special-repair service modes, but
render_npc_item_service still uses a simplified text/button layout, not the
Crystal original. No all-NPC transaction or visual parity acceptance is claimed.
The active client and run-flicker package share the public asset junction;
restarting the client is needed to reliably retry prior failed image loads.

Exporter note: skip-existing export encountered a pre-existing invalid PNG in
Prguse. Fresh export to a separate temporary directory succeeded; only these
four required images were copied. Other source-empty/invalid assets were not
silently replaced or included in this repair.

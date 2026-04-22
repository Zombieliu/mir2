# Crystal MVP Protocol v1

Source extracted from:

- [Packet.cs](/E:/mir2/Crystal/Shared/Packet.cs)
- [ClientPackets.cs](/E:/mir2/Crystal/Shared/ClientPackets.cs)
- [ServerPackets.cs](/E:/mir2/Crystal/Shared/ServerPackets.cs)
- [Enums.cs](/E:/mir2/Crystal/Shared/Enums.cs)
- [SharedData.cs](/E:/mir2/Crystal/Shared/Data/SharedData.cs)

## Framing

Each packet uses this outer frame:

1. `u16_le length`
2. `i16_le packet_id`
3. `payload[length - 4]`

Notes:

- Little-endian is used for numeric primitives.
- Strings use `.NET BinaryWriter.Write(string)` format.
- That means a 7-bit encoded length prefix followed by UTF-8 bytes.
- The base protocol supports compressed packets, but none of the MVP packets below override `Compressed`, so Phase 1 traffic can be treated as uncompressed.

## Client Packet IDs

Phase 1 subset from [ClientPacketIds](/E:/mir2/Crystal/Shared/Enums.cs):

| Id | Name |
| --- | --- |
| 0 | `ClientVersion` |
| 1 | `Disconnect` |
| 2 | `KeepAlive` |
| 3 | `NewAccount` |
| 5 | `Login` |
| 6 | `NewCharacter` |
| 8 | `StartGame` |
| 9 | `LogOut` |
| 10 | `Turn` |
| 11 | `Walk` |
| 12 | `Run` |
| 13 | `Chat` |

## Server Packet IDs

Phase 1 subset from [ServerPacketIds](/E:/mir2/Crystal/Shared/Enums.cs):

| Id | Name |
| --- | --- |
| 0 | `Connected` |
| 1 | `ClientVersion` |
| 2 | `Disconnect` |
| 3 | `KeepAlive` |
| 4 | `NewAccount` |
| 7 | `Login` |
| 8 | `LoginBanned` |
| 9 | `LoginSuccess` |
| 10 | `NewCharacter` |
| 11 | `NewCharacterSuccess` |
| 14 | `StartGame` |
| 15 | `StartGameBanned` |
| 16 | `StartGameDelay` |
| 17 | `MapInformation` |
| 21 | `UserInformation` |
| 23 | `UserLocation` |
| 27 | `ObjectTurn` |
| 28 | `ObjectWalk` |
| 29 | `ObjectRun` |
| 30 | `Chat` |
| 31 | `ObjectChat` |
| 58 | `LogOutSuccess` |
| 59 | `LogOutFailed` |

## Shared Enums

### `MirGender : u8`

| Value | Name |
| --- | --- |
| 0 | `Male` |
| 1 | `Female` |

### `MirClass : u8`

| Value | Name |
| --- | --- |
| 0 | `Warrior` |
| 1 | `Wizard` |
| 2 | `Taoist` |
| 3 | `Assassin` |
| 4 | `Archer` |

### `MirDirection : u8`

| Value | Name |
| --- | --- |
| 0 | `Up` |
| 1 | `UpRight` |
| 2 | `Right` |
| 3 | `DownRight` |
| 4 | `Down` |
| 5 | `DownLeft` |
| 6 | `Left` |
| 7 | `UpLeft` |

### `ChatType : u8`

Important early values:

| Value | Name |
| --- | --- |
| 0 | `Normal` |
| 1 | `Shout` |
| 2 | `System` |
| 3 | `Hint` |
| 4 | `Announcement` |
| 5 | `Group` |
| 6 | `WhisperIn` |
| 7 | `WhisperOut` |
| 8 | `Guild` |

## Shared Structs

### `SelectInfo`

Source: [SharedData.cs](/E:/mir2/Crystal/Shared/Data/SharedData.cs)

1. `i32 index`
2. `string name`
3. `u16 level`
4. `u8 class`
5. `u8 gender`
6. `i64 last_access_binary_datetime`

### `Point`

Used inline, not as a separate packet type:

1. `i32 x`
2. `i32 y`

## Client -> Server Packets

### `ClientVersion` (`id=0`)

1. `i32 hash_length`
2. `u8[hash_length] version_hash`

### `Disconnect` (`id=1`)

No payload.

### `KeepAlive` (`id=2`)

1. `i64 time`

### `NewAccount` (`id=3`)

1. `string account_id`
2. `string password`
3. `i64 birth_date_binary`
4. `string user_name`
5. `string secret_question`
6. `string secret_answer`
7. `string email_address`

### `Login` (`id=5`)

1. `string account_id`
2. `string password`

### `NewCharacter` (`id=6`)

1. `string name`
2. `u8 gender`
3. `u8 class`

### `StartGame` (`id=8`)

1. `i32 character_index`

### `LogOut` (`id=9`)

No payload.

### `Turn` (`id=10`)

1. `u8 direction`

### `Walk` (`id=11`)

1. `u8 direction`

### `Run` (`id=12`)

1. `u8 direction`

### `Chat` (`id=13`)

1. `string message`
2. `i32 linked_item_count`
3. repeated `ChatItem`

Notes:

- `ChatItem` is defined elsewhere in Crystal.
- For Phase 1, a new client can safely send `linked_item_count = 0`.

## Server -> Client Packets

### `Connected` (`id=0`)

No payload.

### `ClientVersion` (`id=1`)

1. `u8 result`

Result values:

- `0` = wrong version
- `1` = accepted

### `Disconnect` (`id=2`)

1. `u8 reason`

Known comments in source:

- `0` = server closing
- `1` = another user
- `2` = packet error
- `3` = server crashed

### `KeepAlive` (`id=3`)

1. `i64 time`

### `NewAccount` (`id=4`)

1. `u8 result`

Result values:

- `0` disabled
- `1` bad account id
- `2` bad password
- `3` bad email
- `4` bad name
- `5` bad question
- `6` bad answer
- `7` account exists
- `8` success

### `Login` (`id=7`)

1. `u8 result`

Result values:

- `0` disabled
- `1` bad account id
- `2` bad password
- `3` account not found
- `4` wrong password

### `LoginBanned` (`id=8`)

1. `string reason`
2. `i64 expiry_binary_datetime`

### `LoginSuccess` (`id=9`)

1. `i32 character_count`
2. repeated `SelectInfo`

### `NewCharacter` (`id=10`)

1. `u8 result`

Result values:

- `0` disabled
- `1` bad character name
- `2` bad gender
- `3` bad class
- `4` max characters reached
- `5` character exists

### `NewCharacterSuccess` (`id=11`)

1. `SelectInfo char_info`

### `StartGame` (`id=14`)

1. `u8 result`
2. `i32 resolution`

Result values:

- `0` disabled
- `1` not logged in
- `2` character not found
- `3` start game error
- `4` success

### `StartGameBanned` (`id=15`)

1. `string reason`
2. `i64 expiry_binary_datetime`

### `StartGameDelay` (`id=16`)

1. `i64 milliseconds`

### `MapInformation` (`id=17`)

1. `i32 map_index`
2. `string file_name`
3. `string title`
4. `u16 mini_map`
5. `u16 big_map`
6. `u8 lights`
7. `u8 flags`
8. `u8 map_dark_light`
9. `u16 music`
10. `u16 weather_particles`

`flags` bit layout:

- bit `0x01` = `lightning`
- bit `0x02` = `fire`

### `UserInformation` (`id=21`)

This is the largest bootstrap packet and should be implemented carefully.

Top-level order:

1. `u32 object_id`
2. `u32 real_id`
3. `string name`
4. `string guild_name`
5. `string guild_rank`
6. `i32 name_colour_argb`
7. `u8 class`
8. `u8 gender`
9. `u16 level`
10. `i32 location_x`
11. `i32 location_y`
12. `u8 direction`
13. `u8 hair`
14. `i32 hp`
15. `i32 mp`
16. `i64 experience`
17. `i64 max_experience`
18. `u16 level_effects`
19. `bool has_hero`
20. `u8 hero_behaviour`
21. `bool has_inventory`
22. optional inventory array
23. `bool has_equipment`
24. optional equipment array
25. `bool has_quest_inventory`
26. optional quest inventory array
27. `u32 gold`
28. `u32 credit`
29. `bool has_expanded_storage`
30. `bool has_storage_password`
31. `bool require_storage_password`
32. `i64 storage_password_last_set_binary`
33. `i64 expanded_storage_expiry_binary`
34. `i32 magic_count`
35. repeated `ClientMagic`
36. `i32 intelligent_creature_count`
37. repeated `ClientIntelligentCreature`
38. `u8 summoned_creature_type`
39. `bool creature_summoned`
40. `bool allow_observe`
41. `bool observer`

Notes:

- Arrays are prefixed by a `bool has_array`, then `i32 length`, then each slot uses `bool present` followed by item payload when present.
- `UserItem`, `ClientMagic`, and `ClientIntelligentCreature` are outside this first MVP extraction and should be documented separately before implementing inventory or skills.
- A Phase 1 client can initially parse only the fields needed for scene bootstrap, but the wire order must still be respected.

### `UserLocation` (`id=23`)

1. `i32 x`
2. `i32 y`
3. `u8 direction`

### `ObjectTurn` (`id=27`)

1. `u32 object_id`
2. `i32 x`
3. `i32 y`
4. `u8 direction`

### `ObjectWalk` (`id=28`)

1. `u32 object_id`
2. `i32 x`
3. `i32 y`
4. `u8 direction`

### `ObjectRun` (`id=29`)

1. `u32 object_id`
2. `i32 x`
3. `i32 y`
4. `u8 direction`

### `Chat` (`id=30`)

1. `string message`
2. `u8 chat_type`

### `ObjectChat` (`id=31`)

1. `u32 object_id`
2. `string text`
3. `u8 chat_type`

### `LogOutSuccess` (`id=58`)

1. `i32 character_count`
2. repeated `SelectInfo`

### `LogOutFailed` (`id=59`)

No payload.

## Recommended First Rust Types

Define these first:

- `ClientPacket`
- `ServerPacket`
- `MirClass`
- `MirGender`
- `MirDirection`
- `ChatType`
- `SelectInfo`
- `MapInformation`
- `UserLocation`
- `ObjectMovement`

Then add custom decoders for:

- `.NET string`
- `BinaryWriter` booleans
- array-with-presence-slot pattern

## Known Gaps

This extraction intentionally does not yet document:

- `UserItem`
- `ClientMagic`
- `ClientIntelligentCreature`
- `ObjectPlayer`
- `ObjectMonster`
- `ObjectNPC`
- `NewItemInfo`, `NewMonsterInfo`, `NewNPCInfo`

Those are the next protocol slice after the first playable map bootstrap is stable.

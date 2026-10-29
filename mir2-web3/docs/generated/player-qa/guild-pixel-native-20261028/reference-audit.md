# Crystal Guild GDI reference audit

Source and GDI reference only; no pixel acceptance claim. Coordinates are relative to GuildDialog.DisplayLocation; add the actual window origin.

| Control | Window origin | Size | Font | Flags | Original line |
|---|---|---|---|---|---|
| NoticeButton | 20,38 | source image/auto |  |  | 138 |
| MembersButton | 91,38 | source image/auto |  |  | 148 |
| StorageButton | 162,38 | source image/auto |  |  | 158 |
| RankButton | 233,38 | source image/auto |  |  | 169 |
| StatusButton | 501,38 | source image/auto |  |  | 180 |
| BuffButton | 430,38 | source image/auto |  |  | 190 |
| CloseButton | 565,4 | source image/auto |  |  | 201 |
| Notice | 13,61 | 322x330 | new Font(Settings.FontName, 8F) |  | 222 |
| NoticeEditButton | 20,402 | source image/auto |  |  | 234 |
| NoticeSaveButton | 20,402 | source image/auto |  |  | 247 |
| NoticeUpButton | 337,61 | 16x14 |  |  | 260 |
| NoticeDownButton | 337,378 | 16x14 |  |  | 280 |
| MembersName[i] | new Point(125, 30 + (i * 15)) | 100x14 | new Font(Settings.FontName, 7F) | WordBreak | 358 |
| MembersStatus[i] | new Point(225, 30 + (i * 15)) | 100x14 | new Font(Settings.FontName, 7F) | WordBreak | 374 |
| MembersUpButton | 337,61 | 16x14 |  |  | 402 |
| MembersDownButton | 337,378 | 16x14 |  |  | 421 |
| MembersShowOfflineButton | 230,370 | source image/auto |  |  | 452 |
| MembersShowOffline | 245,369 | 150x12 | new Font(Settings.FontName, 7F) | WordBreak | 473 |
| StatusHeaders | 362,107 | 75x300 | Settings.FontName 8pt scaled96/DpiX | TextFormatFlags.Right | 519 |
| StatusGuildName | 437,107 | 120x200 | Settings.FontName 8pt scaled96/DpiX | WordBreak | 530 |
| StatusLevel | 437,133 | 120x200 | Settings.FontName 8pt scaled96/DpiX | WordBreak | 539 |
| StatusMembers | 437,159 | 120x200 | Settings.FontName 8pt scaled96/DpiX | WordBreak | 548 |
| StatusExpLabel | 322,405 | 260x15 | Settings.FontName 8pt scaled96/DpiX | TextFormatFlags.VerticalCenter / TextFormatFlags.HorizontalCenter | 569 |
| MembersRecruitName | 395,360 | 130x21 | Settings.FontName 10pt scaled96/DpiX |  | 578 |
| RecruitMemberButton | 525,358 | source image/auto |  |  | 591 |
| RecruitMemberLabel | 391,343 | 150x15 | Settings.FontName 8pt scaled96/DpiX | WordBreak | 604 |
| StorageGoldText | 194,372 | 125x12 | Settings.FontName 8pt scaled96/DpiX | WordBreak | 641 |
| StorageUpButton | 337,61 | 16x14 |  |  | 694 |
| StorageDownButton | 337,378 | 16x14 |  |  | 714 |
| RanksSelectTextL | 42,78 | 150x20 | Settings.FontName 8pt scaled96/DpiX | WordBreak | 760 |
| RanksSelectTextR | 198,78 | 150x20 | Settings.FontName 8pt scaled96/DpiX | WordBreak | 770 |
| RanksName | 42,96 | 130x16 | Settings.FontName 10pt scaled96/DpiX |  | 793 |
| RanksOptionsTexts[i] | new Point(17 + (i % 2 == 0 ? 42 : 202), i % 2 == 0 ? 118 + (i * 20) : 118 + ((i - 1) * 20)) | source image/auto | Settings.FontName 8pt scaled96/DpiX | WordBreak | 859 |
| PointsLeft | 478,64 | 100x20 | Settings.FontName 8pt scaled96/DpiX | TextFormatFlags.HorizontalCenter | 895 |
| UpButton | 563,85 | source image/auto |  |  | 904 |
| DownButton | 563,378 | source image/auto |  |  | 921 |
| HintName | new Point(4, 4) | source image/auto | Settings.FontName 8pt scaled96/DpiX | WordBreak | 1185 |
| RequiredLabel | new Point(4, GameScene.Scene.GuildBuffLabel.DisplayRectangle.Bottom) | source image/auto | Settings.FontName 8pt scaled96/DpiX | WordBreak | 1216 |
| TimeLabel | new Point(4, GameScene.Scene.GuildBuffLabel.DisplayRectangle.Bottom) | source image/auto | Settings.FontName 8pt scaled96/DpiX | WordBreak | 1243 |
| InfoLabel | new Point(4, GameScene.Scene.GuildBuffLabel.DisplayRectangle.Bottom) | source image/auto | Settings.FontName 8pt scaled96/DpiX | WordBreak | 1258 |
| Name | 35,1 | source image/auto | Settings.FontName 8pt scaled96/DpiX | WordBreak | 2206 |
| Info | 35,17 | source image/auto | Settings.FontName 8pt scaled96/DpiX | WordBreak | 2213 |
| Obtained | 140,17 | source image/auto | Settings.FontName 8pt scaled96/DpiX | TextFormatFlags.Right | 2220 |

Exact GDI foreground ink for hahah starts window(441,111), level0(441,137), members1/5(441,163), RecruitMember(395,347). Controls are at437,107 /437,133 /437,159 /391,343 respectively. Default padding+outline offsets account for the difference.

Original Prguse180 is590x432 full bitmap, not an inferred590x430. Individual alpha bounds/true sizes are in JSON. Tabs are original PNG art, not runtime label text.

[Actual GDI glyph bounds](gdi-reference-v1/glyphbounds.json) · [Composed bitmap/label reference](gdi-reference-v1/guild-notice-statistics-label-reference.png) · [C# renderer](render-crystal-guild-reference.cs).

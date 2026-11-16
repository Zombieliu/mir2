# Shared Bevy UI font

`NotoSansCJKsc-Regular.otf` is the unmodified Noto Sans CJK SC Regular font from
[notofonts/noto-cjk](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf).
The source revision is `f8d157532fbfaeda587e826d4cd5b21a49186f7c`.
Its SIL Open Font License is included as `OFL-NotoSansCJK.txt`.

- Bytes: `16437364`
- SHA-256: `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`

The browser Bevy host loads this asset explicitly. It does not depend on
Windows system fonts. Only the opt-in shared quest surface requests it;
it is not a preload for compatibility/mobile startup. Keep the license with
the font when packaging or distributing it. Font load success alone is not
evidence that a particular language or screen is visually accepted.

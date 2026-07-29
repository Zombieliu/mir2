# itch.io HTML5 launcher

Zip `index.html` at the archive root and upload it as an HTML project on itch.io.
The launcher keeps the game runtime on the official Obelisk origin while adding
the `channel=itch` attribution marker. It also offers a new-tab fallback for
browsers that restrict WebAuthn or third-party cookies inside nested iframes.

Example:

```bash
cd distribution/itch
zip -j obelisk-mir2-itch.zip index.html
```

Before publishing, replace the production URL only if the canonical player
domain changes. Do not add Gateway secrets or operator tokens to this package.

"use client";

// QA harness route for scripts/qa-button-sounds.mjs.
//
// Renders the REAL shared <SpriteButton> (the single component behind every original-UI
// "function button" — HUD, login, select, and every dialog) wired to the REAL ORIGINAL_UI
// sprite definitions, with NO gateway, NO web3 login, and NO client shell flow. This gives the
// CDP loop a deterministic target that exercises the exact click→sound path that ships in-game:
//
//   SpriteButton onClick -> playOriginalSoundId(ORIGINAL_SOUND_IDS.uiButtonClick) -> new Audio(...).play()
//
// The regression this guards: every SpriteButton used to play a hard-coded `10100`, which is
// Crystal's SoundList.LoginEffect (the login whoosh), NOT a button click. The fix points it at
// ButtonA (10103, 103.wav). The loop asserts every button here fires 10103 and never 10100.
//
// onClick only bumps a per-key tally on window.__mir2ButtonHarness so the script can confirm the
// click handler actually ran (button wired) in addition to observing the sound it fired.

import { useEffect, useState } from "react";

import { SpriteButton } from "../../components/original-client-overlays";
import { ORIGINAL_UI } from "../../../lib/original-ui";
import type { SpriteState } from "../../../lib/original-ui";

type HarnessButton = { key: string; sprite: SpriteState };

// A representative spread across screens, all rendered through the same SpriteButton. The HUD set
// is exactly the in-game "function buttons" a player clicks; login/select prove the shared button
// behaves identically off the game screen.
const HARNESS_BUTTONS: HarnessButton[] = [
  { key: "hud-gameShop", sprite: ORIGINAL_UI.hud.buttons.gameShop },
  { key: "hud-menu", sprite: ORIGINAL_UI.hud.buttons.menu },
  { key: "hud-character", sprite: ORIGINAL_UI.hud.buttons.character },
  { key: "hud-inventory", sprite: ORIGINAL_UI.hud.buttons.inventory },
  { key: "hud-skill", sprite: ORIGINAL_UI.hud.buttons.skill },
  { key: "hud-quest", sprite: ORIGINAL_UI.hud.buttons.quest },
  { key: "hud-option", sprite: ORIGINAL_UI.hud.buttons.option },
  { key: "login-ok", sprite: ORIGINAL_UI.login.buttons.ok },
  { key: "login-newAccount", sprite: ORIGINAL_UI.login.buttons.newAccount },
  { key: "login-close", sprite: ORIGINAL_UI.login.buttons.close },
  { key: "select-start", sprite: ORIGINAL_UI.select.buttons.start },
  { key: "select-newCharacter", sprite: ORIGINAL_UI.select.buttons.newCharacter },
];

declare global {
  interface Window {
    __mir2ButtonHarness?: { ready: boolean; clicks: Record<string, number> };
  }
}

export default function ButtonSoundsHarnessPage() {
  const [clicks, setClicks] = useState<Record<string, number>>({});

  useEffect(() => {
    window.__mir2ButtonHarness = { ready: true, clicks: {} };
    return () => {
      delete window.__mir2ButtonHarness;
    };
  }, []);

  const recordClick = (key: string) => {
    setClicks((current) => {
      const next = { ...current, [key]: (current[key] ?? 0) + 1 };
      if (window.__mir2ButtonHarness) {
        window.__mir2ButtonHarness.clicks = next;
      }
      return next;
    });
  };

  return (
    <main
      data-mir2-button-sounds-harness="1"
      style={{
        minHeight: "100vh",
        background: "#1b1b1b",
        color: "#ddd",
        font: "13px/1.5 monospace",
        padding: 16,
      }}
    >
      <h1 style={{ fontSize: 16, marginBottom: 4 }}>SpriteButton sound harness</h1>
      <p style={{ marginTop: 0, opacity: 0.7 }}>
        Each button below is the real shared <code>SpriteButton</code>. Clicking one should play
        ButtonA (10103 / 103.wav), never LoginEffect (10100 / 100.wav). Driven by{" "}
        <code>npm run qa:button-sounds</code>.
      </p>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 14, marginTop: 16 }}>
        {HARNESS_BUTTONS.map((button) => (
          <div
            key={button.key}
            data-harness-button={button.key}
            style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 4 }}
          >
            <SpriteButton
              sprite={button.sprite}
              label={button.key}
              onClick={() => recordClick(button.key)}
            />
            <span style={{ opacity: 0.6 }}>
              {button.key} · {clicks[button.key] ?? 0}
            </span>
          </div>
        ))}
      </div>
    </main>
  );
}

"use client";

import type { DisplayKnownSkill, TranslateFn } from "./original-client-types";

export const SKILL_BAR_SLOT_COUNT = 8;

// Crystal binds magic to F1-F8 (and the SkillBarDialog mirrors that). The server
// already assigns each known skill a `hotkey`; skills without one fill the
// remaining slots in order. Passive skills are not placed on the cast bar.
// The keyboard handler and the rendered bar share this assignment so the keys
// and the on-screen slots always match.
export function buildSkillBarSlots(skills: DisplayKnownSkill[]): Array<DisplayKnownSkill | null> {
  const slots: Array<DisplayKnownSkill | null> = new Array(SKILL_BAR_SLOT_COUNT).fill(null);
  const placed = new Set<string>();

  for (const skill of skills) {
    if (
      typeof skill.hotkey === "number" &&
      skill.hotkey >= 1 &&
      skill.hotkey <= SKILL_BAR_SLOT_COUNT &&
      !slots[skill.hotkey - 1]
    ) {
      slots[skill.hotkey - 1] = skill;
      placed.add(skill.key);
    }
  }

  let cursor = 0;
  for (const skill of skills) {
    if (placed.has(skill.key) || skill.castKind === "passive") continue;
    while (cursor < SKILL_BAR_SLOT_COUNT && slots[cursor]) cursor += 1;
    if (cursor >= SKILL_BAR_SLOT_COUNT) break;
    slots[cursor] = skill;
    placed.add(skill.key);
    cursor += 1;
  }

  return slots;
}

export type SkillBarProps = {
  t: TranslateFn;
  knownSkills: DisplayKnownSkill[];
  onCastSkill: (skillKey: string) => void;
};

export function SkillBar({ t, knownSkills, onCastSkill }: SkillBarProps) {
  const slots = buildSkillBarSlots(knownSkills);
  if (!slots.some((skill) => skill)) {
    return null;
  }

  return (
    <section className="skill-bar" aria-label={t("ui.skills", [], "Skills")}>
      {slots.map((skill, index) => {
        const hotkeyLabel = `F${index + 1}`;
        if (!skill) {
          return (
            <div key={`skill-slot-${index}`} className="skill-bar-slot empty" data-skill-slot={index + 1}>
              <span className="skill-bar-key">{hotkeyLabel}</span>
            </div>
          );
        }

        const onCooldown = skill.cooldownRemainingTicks > 0;
        const toggled = skill.castKind === "toggle";
        return (
          <button
            key={`skill-slot-${index}-${skill.key}`}
            type="button"
            className={`skill-bar-slot ${onCooldown ? "cooldown" : ""} ${toggled ? "toggle" : ""}`}
            data-skill-slot={index + 1}
            data-skill-key={skill.key}
            title={`${skill.name}${skill.description ? ` - ${skill.description}` : ""}`}
            onClick={() => onCastSkill(skill.key)}
          >
            <span className="skill-bar-key">{hotkeyLabel}</span>
            <span className="skill-bar-name">{abbreviateSkillName(skill.name)}</span>
            {onCooldown ? <span className="skill-bar-cooldown">{skill.cooldownRemainingTicks}</span> : null}
          </button>
        );
      })}
    </section>
  );
}

function abbreviateSkillName(name: string) {
  const trimmed = name.trim();
  if (trimmed.length <= 8) return trimmed;
  const words = trimmed.split(/\s+/);
  if (words.length > 1) {
    return words
      .map((word) => word[0]?.toUpperCase() ?? "")
      .join("")
      .slice(0, 4);
  }
  return trimmed.slice(0, 7);
}

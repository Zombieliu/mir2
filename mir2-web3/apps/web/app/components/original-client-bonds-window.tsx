"use client";

import { useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** Marriage / lover state, derived from the stage-5 `relationship` slice. */
export type RelationshipSummary = {
  /** Spouse name if married, else empty/undefined. */
  partnerName?: string;
  /** Map the partner is currently on (from LoverUpdate). */
  partnerMap?: string;
  marriedDays?: number;
  /** Whether the viewer accepts incoming marriage proposals. */
  allowMarriage?: boolean;
  /** Name of an incoming marriage/divorce request, if any. */
  pendingRequestFrom?: string;
};

/** Mentor / mentee state, derived from the stage-5 `mentor` slice. */
export type MentorSummary = {
  name?: string;
  level?: number;
  online?: boolean;
  /** Mentee experience accumulated toward graduation. */
  menteeExp?: number;
  allowMentor?: boolean;
  pendingRequestFrom?: string;
};

export type BondsWindowProps = {
  t: TranslateFn;
  relationship: RelationshipSummary | null;
  mentor: MentorSummary | null;
  // Relationship actions.
  onAllowMarriage?: (allow: boolean) => void;
  onProposeMarriage?: (name: string) => void;
  onDivorce?: () => void;
  // Mentor actions.
  onAllowMentor?: (allow: boolean) => void;
  onAddMentor?: (name: string) => void;
  onCancelMentor?: () => void;
  onClose: () => void;
};

type BondsTab = "relationship" | "mentor";

const FRAME = ORIGINAL_UI.character;

export function BondsWindow({
  t,
  relationship,
  mentor,
  onAllowMarriage,
  onProposeMarriage,
  onDivorce,
  onAllowMentor,
  onAddMentor,
  onCancelMentor,
  onClose,
}: BondsWindowProps) {
  const [tab, setTab] = useState<BondsTab>("relationship");
  const [proposeName, setProposeName] = useState("");
  const [mentorName, setMentorName] = useState("");

  const partner = relationship?.partnerName?.trim() ?? "";
  const married = partner.length > 0;
  const allowMarriage = relationship?.allowMarriage ?? true;

  const mentorOf = mentor?.name?.trim() ?? "";
  const hasMentorLink = mentorOf.length > 0;
  const allowMentor = mentor?.allowMentor ?? true;

  return (
    <section
      aria-label={t("ui.relationship", [], "Bonds")}
      data-bonds-tab={tab}
      data-bonds-married={married ? "1" : "0"}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.page} src={FRAME.pages.char} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.relationship", [], "Bonds")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.relationship", [], "Bonds")}>
        <button
          type="button"
          role="tab"
          data-bonds-tab="relationship"
          aria-selected={tab === "relationship"}
          onClick={() => setTab("relationship")}
          style={{ ...style.tab, ...(tab === "relationship" ? style.tabActive : null) }}
        >
          {t("ui.relationship", [], "Marriage")}
        </button>
        <button
          type="button"
          role="tab"
          data-bonds-tab="mentor"
          aria-selected={tab === "mentor"}
          onClick={() => setTab("mentor")}
          style={{ ...style.tab, ...(tab === "mentor" ? style.tabActive : null) }}
        >
          {t("ui.mentor", [], "Mentor")}
        </button>
      </div>

      {tab === "relationship" ? (
        <div style={style.body}>
          <div style={style.statusCard} data-relationship-partner={partner}>
            <div style={style.statusName}>{married ? partner : t("ui.bondsSingle", [], "Single")}</div>
            <div style={style.statusMeta}>
              {married ? t("ui.bondsMarried", [], "Married") : t("ui.bondsUnmarried", [], "No spouse")}
            </div>
          </div>

          <Info label={t("ui.bondsPartner", [], "Spouse")} value={married ? partner : "-"} />
          {relationship?.partnerMap ? (
            <Info label={t("ui.bondsLocation", [], "Location")} value={relationship.partnerMap} />
          ) : null}
          {typeof relationship?.marriedDays === "number" ? (
            <Info
              label={t("ui.bondsDays", [], "Days Married")}
              value={String(Math.max(0, relationship.marriedDays))}
            />
          ) : null}
          <Info
            label={t("ui.bondsRequest", [], "Pending")}
            value={relationship?.pendingRequestFrom?.trim() || t("ui.bondsNoRequest", [], "None")}
          />

          <div style={style.toggleRow}>
            <span style={style.toggleLabel}>{t("ui.bondsAllow", [], "Allow Proposals")}</span>
            <button
              type="button"
              disabled={!onAllowMarriage}
              aria-pressed={allowMarriage}
              onClick={() => onAllowMarriage?.(!allowMarriage)}
              style={{ ...style.toggleButton, ...(!onAllowMarriage ? style.actionButtonDisabled : null) }}
            >
              {allowMarriage ? t("ui.on", [], "On") : t("ui.off", [], "Off")}
            </button>
          </div>

          <div style={style.spacer} />

          <form
            style={style.inputRow}
            onSubmit={(event) => {
              event.preventDefault();
              const trimmed = proposeName.trim();
              if (trimmed) {
                onProposeMarriage?.(trimmed);
                setProposeName("");
              }
            }}
          >
            <input
              style={style.input}
              value={proposeName}
              onChange={(event) => setProposeName(event.target.value)}
              placeholder={t("ui.bondsProposePlaceholder", [], "Partner name")}
              aria-label={t("ui.bondsPropose", [], "Propose")}
              autoComplete="off"
              spellCheck={false}
              disabled={married}
            />
            <button
              type="submit"
              disabled={married || !onProposeMarriage || proposeName.trim().length === 0}
              style={{
                ...style.actionButton,
                flex: "0 0 84px",
                ...(married || !onProposeMarriage || proposeName.trim().length === 0 ? style.actionButtonDisabled : null),
              }}
            >
              {t("ui.bondsPropose", [], "Propose")}
            </button>
          </form>
          <button
            type="button"
            disabled={!married || !onDivorce}
            style={{ ...style.actionButton, width: "100%", ...(!married || !onDivorce ? style.actionButtonDisabled : null) }}
            onClick={() => onDivorce?.()}
          >
            {t("ui.bondsDivorce", [], "Divorce")}
          </button>
        </div>
      ) : (
        <div style={style.body}>
          <div style={style.statusCard} data-mentor-name={mentorOf}>
            <div style={style.statusName}>
              {hasMentorLink ? mentorOf : t("ui.bondsNoMentor", [], "No Mentor")}
            </div>
            <div style={style.statusMeta}>
              {hasMentorLink
                ? mentor?.online
                  ? t("ui.guildStateOnline", [], "Online")
                  : t("ui.guildStateOffline", [], "Offline")
                : t("ui.bondsMentorOpen", [], "Open to mentoring")}
            </div>
          </div>

          <Info label={t("ui.bondsMentorName", [], "Mentor")} value={hasMentorLink ? mentorOf : "-"} />
          <Info
            label={t("ui.guildLevel", [], "Level")}
            value={mentor?.level ? String(mentor.level) : "-"}
          />
          <Info
            label={t("ui.bondsMenteeExp", [], "Mentee EXP")}
            value={typeof mentor?.menteeExp === "number" ? String(Math.max(0, mentor.menteeExp)) : "0"}
          />
          <Info
            label={t("ui.bondsRequest", [], "Pending")}
            value={mentor?.pendingRequestFrom?.trim() || t("ui.bondsNoRequest", [], "None")}
          />

          <div style={style.toggleRow}>
            <span style={style.toggleLabel}>{t("ui.bondsAllowMentor", [], "Allow Requests")}</span>
            <button
              type="button"
              disabled={!onAllowMentor}
              aria-pressed={allowMentor}
              onClick={() => onAllowMentor?.(!allowMentor)}
              style={{ ...style.toggleButton, ...(!onAllowMentor ? style.actionButtonDisabled : null) }}
            >
              {allowMentor ? t("ui.on", [], "On") : t("ui.off", [], "Off")}
            </button>
          </div>

          <div style={style.spacer} />

          <form
            style={style.inputRow}
            onSubmit={(event) => {
              event.preventDefault();
              const trimmed = mentorName.trim();
              if (trimmed) {
                onAddMentor?.(trimmed);
                setMentorName("");
              }
            }}
          >
            <input
              style={style.input}
              value={mentorName}
              onChange={(event) => setMentorName(event.target.value)}
              placeholder={t("ui.bondsMentorPlaceholder", [], "Mentor / mentee name")}
              aria-label={t("ui.bondsAddMentor", [], "Add")}
              autoComplete="off"
              spellCheck={false}
            />
            <button
              type="submit"
              disabled={!onAddMentor || mentorName.trim().length === 0}
              style={{
                ...style.actionButton,
                flex: "0 0 84px",
                ...(!onAddMentor || mentorName.trim().length === 0 ? style.actionButtonDisabled : null),
              }}
            >
              {t("ui.bondsAddMentor", [], "Add")}
            </button>
          </form>
          <button
            type="button"
            disabled={!hasMentorLink || !onCancelMentor}
            style={{ ...style.actionButton, width: "100%", ...(!hasMentorLink || !onCancelMentor ? style.actionButtonDisabled : null) }}
            onClick={() => onCancelMentor?.()}
          >
            {t("ui.bondsCancelMentor", [], "Break Link")}
          </button>
        </div>
      )}
    </section>
  );
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div style={style.info}>
      <span style={style.infoLabel}>{label}</span>
      <span style={style.infoValue}>{value}</span>
    </div>
  );
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 400,
    top: 130,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 30,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  page: { position: "absolute", left: 8, top: 90, pointerEvents: "none", opacity: 0.22 },
  titleText: {
    position: "absolute",
    left: 16,
    top: 8,
    height: 16,
    lineHeight: "16px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  close: { position: "absolute", left: 238, top: 4 },
  tabs: { position: "absolute", left: 12, top: 28, width: 240, display: "flex", gap: 3 },
  tab: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "3px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  body: {
    position: "absolute",
    left: 12,
    top: 54,
    width: 240,
    height: 314,
    display: "flex",
    flexDirection: "column",
    gap: 6,
  },
  statusCard: {
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.45), rgba(28, 17, 9, 0.55))",
    padding: "8px 10px",
  },
  statusName: { color: "#f8e6bb", fontSize: 14, fontWeight: 700 },
  statusMeta: { fontSize: 11, color: "#cbb38a", marginTop: 2 },
  info: {
    display: "flex",
    justifyContent: "space-between",
    gap: 8,
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(20, 13, 7, 0.45)",
    padding: "4px 8px",
    fontSize: 11,
  },
  infoLabel: { color: "#a89568" },
  infoValue: { color: "#f0d69b", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  toggleRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 },
  toggleLabel: { fontSize: 10, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  toggleButton: {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "2px 16px",
    fontSize: 11,
    cursor: "pointer",
  },
  spacer: { flex: 1 },
  inputRow: { display: "flex", gap: 6 },
  input: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "4px 8px",
    fontSize: 11,
  },
  actionButton: {
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "5px 10px",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};

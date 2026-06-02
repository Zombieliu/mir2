"use client";

import { useState } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type DisplayMailMessageLike = {
  id?: number;
  from?: string;
  to?: string;
  subject?: string;
  body?: string;
  gold?: number;
  items?: string[];
  claimed?: boolean;
  deleted?: boolean;
};

type DisplayLogLineLike = {
  text: string;
  tone: "chat" | "system" | "network";
};

type DisplayNpcDialogLike = {
  npcObjectId: string;
  npcName: string;
  title: string;
  body: string[];
  footer: string;
  links: Array<{
    text: string;
    target: string;
  }>;
  input?: {
    target: string;
    prompt: string;
  } | null;
};

export type MailPanelProps = {
  t: TranslateFn;
  mail: DisplayMailMessageLike[];
  onClaim: (mailId: number) => void;
  onDelete: (mailId: number) => void;
  onClose: () => void;
};

export function MailPanel({ t, mail, onClaim, onDelete, onClose }: MailPanelProps) {
  const entries = mail.filter((message) => !message.deleted);
  const visibleEntries = entries.slice(0, 10);
  const selectedEntry = visibleEntries.find((entry) => entry.id !== undefined) ?? visibleEntries[0] ?? null;
  const pageCount = Math.max(1, Math.ceil(entries.length / 10));

  return (
    <section className="mail-panel">
      <img className="mail-frame" src={ORIGINAL_UI.mail.frame} alt="" draggable={false} />
      <img className="mail-title-image" src={ORIGINAL_UI.mail.title} alt="" draggable={false} />
      <div className="mail-close">
        <SpriteButton sprite={ORIGINAL_UI.mail.closeButton} label={t("ui.close")} onClick={onClose} />
      </div>
      <div className="mail-help">
        <SpriteButton sprite={ORIGINAL_UI.mail.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
      </div>
      <div className="mail-header type">{t("client.Type", [], "Type")}</div>
      <div className="mail-header sender">{t("client.Sender", [], "Sender")}</div>
      <div className="mail-header message">{t("client.Message", [], "Message")}</div>
      {visibleEntries.map((entry, index) => (
        <MailListRow
          key={`mail-row-${entry.id ?? index}`}
          entry={entry}
          index={index}
          selected={index === 0}
          onClaim={onClaim}
          onDelete={onDelete}
        />
      ))}
      <div className="mail-page-previous">
        <SpriteButton sprite={ORIGINAL_UI.mail.previousButton} label={t("ui.previous", [], "Previous")} onClick={() => undefined} />
      </div>
      <div className="overlay-panel-foot mail-page-label">{`1 / ${pageCount}`}</div>
      <div className="mail-page-next">
        <SpriteButton sprite={ORIGINAL_UI.mail.nextButton} label={t("ui.next", [], "Next")} onClick={() => undefined} />
      </div>
      <div className="mail-action send"><SpriteButton sprite={ORIGINAL_UI.mail.sendButton} label={t("client.Send", [], "Send")} onClick={() => undefined} /></div>
      <div className="mail-action reply"><SpriteButton sprite={ORIGINAL_UI.mail.replyButton} label={t("client.Reply", [], "Reply")} onClick={() => undefined} /></div>
      <div className="mail-action read">
        <SpriteButton
          sprite={ORIGINAL_UI.mail.readButton}
          label={t("client.Read", [], "Read")}
          onClick={() => selectedEntry?.id !== undefined && !selectedEntry.claimed ? onClaim(selectedEntry.id) : undefined}
        />
      </div>
      <div className="mail-action delete">
        <SpriteButton
          sprite={ORIGINAL_UI.mail.deleteButton}
          label={t("client.Delete", [], "Delete")}
          onClick={() => selectedEntry?.id !== undefined ? onDelete(selectedEntry.id) : undefined}
        />
      </div>
      <div className="mail-action block disabled"><SpriteButton sprite={ORIGINAL_UI.mail.blockListButton} label={t("client.BlockList", [], "Block List")} onClick={() => undefined} /></div>
      <div className="mail-action bug disabled"><SpriteButton sprite={ORIGINAL_UI.mail.bugReportButton} label={t("client.ReportBug", [], "Report Bug")} onClick={() => undefined} /></div>
      <div className="overlay-panel-list mail-legacy-list" hidden>
        {entries.length ? (
          entries.map((entry, index) => (
            <div key={`mail-${entry.id ?? index}`} className="overlay-panel-row">
              <strong>{entry.subject ?? t("client.Mail", [], "Mail")}</strong>
              <span>{`${entry.from ?? "System"} -> ${entry.to ?? "You"}`}</span>
              <span>{entry.body ?? ""}</span>
              <span>
                {[
                  entry.gold ? `${entry.gold} Gold` : null,
                  entry.items?.length ? `${entry.items.join(", ")}` : null,
                  entry.claimed ? "Claimed" : "Unclaimed",
                ]
                  .filter(Boolean)
                  .join(" \u00b7 ")}
              </span>
              <div className="overlay-panel-actions">
                <button
                  type="button"
                  disabled={entry.claimed || entry.id === undefined}
                  onClick={() => entry.id !== undefined && onClaim(entry.id)}
                >
                  Claim
                </button>
                <button
                  type="button"
                  disabled={entry.id === undefined}
                  onClick={() => entry.id !== undefined && onDelete(entry.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          ))
        ) : (
          <div className="overlay-panel-empty">No mail</div>
        )}
      </div>
      <div className="overlay-panel-foot mail-legacy-foot">{`${entries.length}/${mail.length}`}</div>
    </section>
  );
}

function MailListRow({
  entry,
  index,
  selected,
  onClaim,
  onDelete,
}: {
  entry: DisplayMailMessageLike;
  index: number;
  selected: boolean;
  onClaim: (mailId: number) => void;
  onDelete: (mailId: number) => void;
}) {
  const hasParcel = !entry.claimed && (Boolean(entry.gold) || Boolean(entry.items?.length));
  const icon = entry.gold && !entry.items?.length ? ORIGINAL_UI.mail.icons.gold : ORIGINAL_UI.mail.icons.letter;
  const sender = entry.from ?? "System";
  const message = (entry.body || entry.subject || "").replace(/\s+/g, " ");

  return (
    <div
      role="button"
      tabIndex={0}
      className="overlay-panel-row mail-row"
      style={{ top: 55 + index * 33 }}
      onDoubleClick={() => entry.id !== undefined && !entry.claimed && onClaim(entry.id)}
      onKeyDown={(event) => {
        if ((event.key === "Enter" || event.key === " ") && entry.id !== undefined && !entry.claimed) {
          event.preventDefault();
          onClaim(entry.id);
        }
      }}
    >
      {selected ? <img className="mail-row-selected" src={ORIGINAL_UI.mail.icons.selected} alt="" draggable={false} /> : null}
      <span className="mail-row-icon-area">
        <img className="mail-row-icon" src={icon} alt="" draggable={false} />
        {!entry.claimed ? <img className={`mail-row-flag unread ${hasParcel ? "second" : ""}`} src={ORIGINAL_UI.mail.icons.unread} alt="" draggable={false} /> : null}
        {hasParcel ? <img className="mail-row-flag parcel" src={ORIGINAL_UI.mail.icons.parcel} alt="" draggable={false} /> : null}
      </span>
      <span className="mail-row-sender">{sender}</span>
      <span className="mail-row-message">{message}</span>
      <span className="overlay-panel-actions mail-row-actions">
        <button
          type="button"
          disabled={entry.claimed || entry.id === undefined}
          onClick={(event) => {
            event.stopPropagation();
            if (entry.id !== undefined) onClaim(entry.id);
          }}
        >
          Claim
        </button>
        <button
          type="button"
          disabled={entry.id === undefined}
          onClick={(event) => {
            event.stopPropagation();
            if (entry.id !== undefined) onDelete(entry.id);
          }}
        >
          Delete
        </button>
      </span>
    </div>
  );
}

export type ReportPanelProps = {
  t: TranslateFn;
  logs: DisplayLogLineLike[];
  onClose: () => void;
};

export function ReportPanel({ t, logs, onClose }: ReportPanelProps) {
  const lines = logs.filter((line) => line.tone !== "network").slice(0, 6);

  return (
    <section className="overlay-panel report-panel">
      <div className="overlay-panel-head">
        <strong>{t("ui.report")}</strong>
        <button type="button" onClick={onClose}>
          {t("ui.close")}
        </button>
      </div>
      <div className="overlay-panel-list">
        {lines.map((line, index) => (
          <div key={`report-${index}`} className="overlay-panel-row">
            {trimLogTimestamp(line.text)}
          </div>
        ))}
      </div>
      <div className="overlay-panel-foot">{`${lines.length}/6`}</div>
    </section>
  );
}

export type NpcDialogPanelProps = {
  t: TranslateFn;
  dialog: DisplayNpcDialogLike;
  onClose: () => void;
  onSelectTarget: (target: string) => void;
  onSubmitInput: (value: string) => void;
  /** Optional NPC portrait image URL, shown in the header. */
  portrait?: string | null;
};

/** Classify a dialog option for icon hinting (matches Crystal @-targets). */
type NpcLinkKind = "default" | "buy" | "sell" | "repair" | "quest" | "exit" | "back";

function npcLinkKind(text: string, target: string): NpcLinkKind {
  const key = `${target} ${text}`.toLowerCase();
  if (/@exit\b|\bexit\b|\bgoodbye\b|\bleave\b/.test(key)) return "exit";
  if (/@back\b|\bback\b|\bmain\b/.test(key)) return "back";
  if (/buysell|@buy\b|\bbuy\b|\bgoods\b|\bshop\b|\bstore\b/.test(key)) return "buy";
  if (/@sell\b|\bsell\b/.test(key)) return "sell";
  if (/@?repair|\brepair\b/.test(key)) return "repair";
  if (/quest|task|mission|@accept|@finish|@complete|@share/.test(key)) return "quest";
  return "default";
}

/** A small leading glyph for the option, echoing the original icon affordances. */
function npcLinkGlyph(kind: NpcLinkKind): string {
  switch (kind) {
    case "buy":
      return "▸"; // ▸ buy/goods
    case "sell":
      return "▾"; // ▾ sell
    case "repair":
      return "⚒"; // ⚒ repair
    case "quest":
      return "✧"; // ✧ quest
    case "exit":
      return "✕"; // ✕ exit
    case "back":
      return "‹"; // ‹ back
    default:
      return "•"; // • default
  }
}

/** Extract a gold amount mentioned in an option's text, if any. */
function npcLinkGold(text: string): number | null {
  const match = text.replace(/,/g, "").match(/(\d{2,})\s*(?:gold|gp)\b/i);
  if (!match) return null;
  const value = Number.parseInt(match[1], 10);
  return Number.isFinite(value) ? value : null;
}

export function NpcDialogPanel({
  t,
  dialog,
  onClose,
  onSelectTarget,
  onSubmitInput,
  portrait,
}: NpcDialogPanelProps) {
  const [inputValue, setInputValue] = useState("");
  const bodyLines = dialog.body.map(stripCrystalDialogMarkup).filter((line) => line.trim().length > 0);
  const title = stripCrystalDialogMarkup(dialog.title || dialog.npcName);
  const footer = stripCrystalDialogMarkup(dialog.footer);

  return (
    <section className="npc-dialog-panel">
      <div className="npc-dialog-head">
        {portrait ? (
          <img
            src={portrait}
            alt=""
            draggable={false}
            style={{ position: "absolute", left: 4, top: 2, width: 20, height: 20, imageRendering: "pixelated" }}
          />
        ) : null}
        <strong>{title}</strong>
        <div className="npc-dialog-actions">
          <SpriteButton sprite={ORIGINAL_UI.mail.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
          <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.close")} onClick={onClose} />
        </div>
      </div>
      <div className="npc-dialog-body" style={{ maxHeight: 168, overflowY: "auto" }}>
        {bodyLines.map((line, index) => (
          <p key={`${dialog.npcObjectId}-${index}`}>{line}</p>
        ))}
        {dialog.links.length ? (
          <div className="npc-dialog-links">
            {dialog.links.map((link, index) => {
              const label = stripCrystalDialogMarkup(link.text);
              const kind = npcLinkKind(label, link.target);
              const gold = npcLinkGold(label);
              return (
                <button
                  key={`${dialog.npcObjectId}-link-${index}-${link.target}`}
                  type="button"
                  data-target={link.target}
                  data-link-kind={kind}
                  onClick={() => onSelectTarget(link.target)}
                >
                  <span aria-hidden style={{ display: "inline-block", width: 14, color: "#caa64a" }}>
                    {npcLinkGlyph(kind)}
                  </span>
                  {label}
                  {gold !== null ? (
                    <span style={{ marginLeft: 6, color: "#f0d98a" }}>{`(${gold.toLocaleString("en-US")} ${t("client.Gold", [], "Gold")})`}</span>
                  ) : null}
                  {kind === "quest" ? (
                    <span style={{ marginLeft: 6, color: "#7be07a" }}>{t("ui.questTag", [], "[Quest]")}</span>
                  ) : null}
                </button>
              );
            })}
          </div>
        ) : null}
      </div>
      {dialog.input ? (
        <form
          className="npc-dialog-input-form"
          onSubmit={(event) => {
            event.preventDefault();
            onSubmitInput(inputValue);
            setInputValue("");
          }}
        >
          <label>
            <span>{stripCrystalDialogMarkup(dialog.input.prompt)}</span>
            <input
              value={inputValue}
              onChange={(event) => setInputValue(event.target.value)}
              autoComplete="off"
              autoFocus
            />
          </label>
          <button type="submit">{t("ui.confirm", [], "Confirm")}</button>
        </form>
      ) : null}
      {footer ? <div className="npc-dialog-footer">{footer}</div> : null}
    </section>
  );
}

function trimLogTimestamp(text: string) {
  return text.replace(/^\[\d{1,2}:\d{2}:\d{2}(?:\s?[AP]M)?\]\s*/i, "");
}

function stripCrystalDialogMarkup(text: string) {
  return text
    .replace(/\{\/?[A-Z]+\}/gi, "")
    .replace(/<\$[^>]+>/g, "")
    .replace(/%[A-Z0-9_()]+/gi, "")
    .replace(/\s{2,}/g, " ")
    .trim();
}

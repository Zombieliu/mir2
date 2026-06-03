"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/**
 * Mirrors the stage-5 `mail` slice the page maintains from the
 * `MailSendRequest` / `NewMail` / `MailLockedItem` / `ParcelCollected` packets
 * (see Crystal `MailListDialog` / `MailReadParcelDialog`):
 *
 *   { id, from, to, subject, body, gold, items, claimed, deleted }
 *
 * Every field is optional so any compatible payload (including the very small
 * objects the loosely-typed `stage5Systems.mail` slice produces) keeps working.
 * This is the same shape as `DisplayMailMessage` in `original-client-types`,
 * declared locally so the window stays self-contained / does not depend on a
 * lib import the host might own.
 */
export type MailMessageSummary = {
  id?: number;
  from?: string;
  to?: string;
  subject?: string;
  body?: string;
  gold?: number;
  items?: string[];
  claimed?: boolean;
  deleted?: boolean;
  /** Optional unread flag; defaults to `!claimed` when absent. */
  read?: boolean;
  /** Optional ISO / display date string, e.g. "2026-06-02 14:05". */
  date?: string;
  /** Optional locked flag (parcel cannot be claimed yet) — renders a badge. */
  locked?: boolean;
};

/** Payload the compose form emits; consumed by an optional `onSendMail`. */
export type MailComposeDraft = {
  to: string;
  subject: string;
  body: string;
  /** Gold to attach (0 / undefined = none). */
  gold?: number;
  /** Item references to attach (names or ids the host can resolve). */
  items?: string[];
};

export type MailWindowProps = {
  t: TranslateFn;
  /** Inbox messages; `deleted` entries are filtered out for display. */
  mail?: MailMessageSummary[];
  /** Player gold, shown next to the compose gold field as a max hint. */
  gold?: number;
  /**
   * Candidate items the player could attach to a parcel (inventory items).
   * Rendered as a pick-list in the compose form; absent → no attach picker.
   */
  attachableItems?: Array<{ key: string; name: string; quantity?: number }>;
  /** Open / read a message (mark read). */
  onOpen?: (mailId: number) => void;
  /** Claim the gold / items attached to a message. */
  onClaimAttachment?: (mailId: number) => void;
  /** Delete a message from the inbox. */
  onDeleteMail?: (mailId: number) => void;
  /** Send a newly composed letter / parcel. */
  onSendMail?: (draft: MailComposeDraft) => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.mail;
const ICONS = ORIGINAL_UI.mail.icons;

type MailView = "inbox" | "read" | "compose";

function senderName(message: MailMessageSummary) {
  return message.from?.trim() || "System";
}

function isUnread(message: MailMessageSummary) {
  if (typeof message.read === "boolean") return !message.read;
  // Crystal flags letters unread until opened; parcels until claimed. With no
  // explicit flag we treat an unclaimed message as unread.
  return message.claimed !== true;
}

function hasAttachment(message: MailMessageSummary) {
  return Boolean(message.gold && message.gold > 0) || Boolean(message.items?.length);
}

function attachmentSummary(t: TranslateFn, message: MailMessageSummary) {
  const parts: string[] = [];
  if (message.gold && message.gold > 0) {
    parts.push(`${formatNumber(message.gold)} ${t("client.Gold", [], "Gold")}`);
  }
  if (message.items?.length) {
    parts.push(message.items.join(", "));
  }
  return parts.join(" · ");
}

function formatNumber(value: number) {
  return Math.max(0, Math.trunc(value)).toLocaleString("en-US");
}

export function MailWindow({
  t,
  mail,
  gold,
  attachableItems,
  onOpen,
  onClaimAttachment,
  onDeleteMail,
  onSendMail,
  onClose,
}: MailWindowProps) {
  const entries = useMemo(
    () => (mail ?? []).filter((message) => !message.deleted),
    [mail],
  );

  const [view, setView] = useState<MailView>("inbox");
  const [selectedId, setSelectedId] = useState<number | null>(null);

  // Compose form state.
  const [composeTo, setComposeTo] = useState("");
  const [composeSubject, setComposeSubject] = useState("");
  const [composeBody, setComposeBody] = useState("");
  const [composeGold, setComposeGold] = useState("");
  const [composeItems, setComposeItems] = useState<string[]>([]);

  const selected = useMemo(
    () => entries.find((entry) => entry.id !== undefined && entry.id === selectedId) ?? null,
    [entries, selectedId],
  );

  // Keep the selection valid as the inbox changes; fall back to the first item.
  useEffect(() => {
    if (view !== "compose" && selected === null && entries.length > 0) {
      const first = entries.find((entry) => entry.id !== undefined);
      setSelectedId(first?.id ?? null);
    }
  }, [entries, selected, view]);

  const unreadCount = entries.filter(isUnread).length;

  function openMessage(message: MailMessageSummary) {
    if (message.id === undefined) return;
    setSelectedId(message.id);
    setView("read");
    if (isUnread(message)) {
      onOpen?.(message.id);
    }
  }

  function startCompose(replyTo?: MailMessageSummary) {
    setComposeTo(replyTo ? senderName(replyTo) : "");
    setComposeSubject(replyTo?.subject ? `RE: ${replyTo.subject}` : "");
    setComposeBody("");
    setComposeGold("");
    setComposeItems([]);
    setView("compose");
  }

  function submitCompose() {
    if (!onSendMail) return;
    const to = composeTo.trim();
    if (!to) return;
    const parsedGold = Number.parseInt(composeGold, 10);
    onSendMail({
      to,
      subject: composeSubject.trim(),
      body: composeBody,
      gold: Number.isFinite(parsedGold) && parsedGold > 0 ? parsedGold : undefined,
      items: composeItems.length ? composeItems : undefined,
    });
    setView("inbox");
  }

  const composeValid = composeTo.trim().length > 0;

  return (
    <section
      aria-label={t("client.Mail", [], "Mail")}
      data-mail-view={view}
      data-mail-unread={unreadCount}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.title} src={FRAME.title} alt="" draggable={false} />
      <div style={style.titleText}>{t("client.Mail", [], "Mail")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      {/* Sub-view tabs ----------------------------------------------------- */}
      <div style={style.tabs}>
        <button
          type="button"
          style={tabStyle(view === "inbox")}
          onClick={() => setView("inbox")}
        >
          {`${t("ui.mailInbox", [], "Inbox")}${unreadCount ? ` (${unreadCount})` : ""}`}
        </button>
        <button
          type="button"
          style={{ ...tabStyle(view === "compose"), ...(!onSendMail ? style.disabled : null) }}
          disabled={!onSendMail}
          title={!onSendMail ? t("ui.mailSendDisabled", [], "Sending is unavailable.") : undefined}
          onClick={() => startCompose()}
        >
          {t("client.SendMail", [], "Send Mail")}
        </button>
      </div>

      {view === "inbox" ? (
        <MailInbox
          t={t}
          entries={entries}
          selectedId={selectedId}
          onSelect={(id) => setSelectedId(id)}
          onOpen={openMessage}
          onClaim={onClaimAttachment}
          onDelete={onDeleteMail}
        />
      ) : null}

      {view === "read" ? (
        <MailReadPane
          t={t}
          message={selected}
          onClaim={onClaimAttachment}
          onDelete={onDeleteMail}
          onReply={onSendMail ? () => selected && startCompose(selected) : undefined}
          onBack={() => setView("inbox")}
        />
      ) : null}

      {view === "compose" ? (
        <MailComposeForm
          t={t}
          to={composeTo}
          subject={composeSubject}
          body={composeBody}
          goldText={composeGold}
          gold={gold}
          attachableItems={attachableItems}
          selectedItems={composeItems}
          canSend={Boolean(onSendMail) && composeValid}
          sendEnabled={Boolean(onSendMail)}
          onToChange={setComposeTo}
          onSubjectChange={setComposeSubject}
          onBodyChange={setComposeBody}
          onGoldChange={setComposeGold}
          onToggleItem={(key) =>
            setComposeItems((current) =>
              current.includes(key) ? current.filter((entry) => entry !== key) : [...current, key],
            )
          }
          onSubmit={submitCompose}
          onCancel={() => setView("inbox")}
        />
      ) : null}
    </section>
  );
}

/* ------------------------------------------------------------------------- */
/* Inbox list                                                                */
/* ------------------------------------------------------------------------- */

function MailInbox({
  t,
  entries,
  selectedId,
  onSelect,
  onOpen,
  onClaim,
  onDelete,
}: {
  t: TranslateFn;
  entries: MailMessageSummary[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onOpen: (message: MailMessageSummary) => void;
  onClaim?: (mailId: number) => void;
  onDelete?: (mailId: number) => void;
}) {
  return (
    <>
      <div style={style.listHeader}>
        <span style={{ ...style.colType }}>{t("client.Type", [], "Type")}</span>
        <span style={{ ...style.colSender }}>{t("client.Sender", [], "Sender")}</span>
        <span style={{ ...style.colMessage }}>{t("client.Message", [], "Message")}</span>
      </div>
      <div style={style.list}>
        {entries.length ? (
          entries.map((entry, index) => (
            <MailListRow
              key={`mail-${entry.id ?? index}`}
              t={t}
              entry={entry}
              selected={entry.id !== undefined && entry.id === selectedId}
              onSelect={() => entry.id !== undefined && onSelect(entry.id)}
              onOpen={() => onOpen(entry)}
              onClaim={onClaim}
              onDelete={onDelete}
            />
          ))
        ) : (
          <div style={style.empty}>{t("ui.mailEmpty", [], "No mail.")}</div>
        )}
      </div>
      <div style={style.footer}>{t("ui.mailCount", [entries.length], `${entries.length} message(s)`)}</div>
    </>
  );
}

function MailListRow({
  t,
  entry,
  selected,
  onSelect,
  onOpen,
  onClaim,
  onDelete,
}: {
  t: TranslateFn;
  entry: MailMessageSummary;
  selected: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onClaim?: (mailId: number) => void;
  onDelete?: (mailId: number) => void;
}) {
  const unread = isUnread(entry);
  const parcel = hasAttachment(entry);
  const goldOnly = Boolean(entry.gold && entry.gold > 0) && !entry.items?.length;
  const icon = goldOnly ? ICONS.gold : ICONS.letter;
  const message = (entry.subject || entry.body || "").replace(/\s+/g, " ").trim();
  const canClaim = Boolean(onClaim) && parcel && !entry.claimed && !entry.locked;
  const canDelete = Boolean(onDelete) && entry.id !== undefined;

  return (
    <div
      role="button"
      tabIndex={0}
      aria-label={`${senderName(entry)}: ${message}`}
      style={{ ...style.row, ...(selected ? style.rowSelected : null) }}
      onClick={onSelect}
      onDoubleClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onOpen();
        }
      }}
    >
      <span style={style.rowIconArea}>
        <img style={style.rowIcon} src={icon} alt="" draggable={false} />
        {unread ? <img style={style.rowFlag} src={ICONS.unread} alt="" title={t("client.MailHeaderNew", [], "[New]")} draggable={false} /> : null}
        {entry.locked ? <img style={style.rowFlagAlt} src={ICONS.locked} alt="" title={t("ui.mailLocked", [], "Locked")} draggable={false} /> : null}
        {parcel && !entry.locked ? <img style={style.rowFlagAlt} src={ICONS.parcel} alt="" title={t("ui.mailHasAttachment", [], "Attachment")} draggable={false} /> : null}
      </span>
      <span style={{ ...style.rowSender, ...(unread ? style.unreadText : null) }}>{senderName(entry)}</span>
      <span style={style.rowMessage}>
        <span style={unread ? style.unreadText : undefined}>{message || t("client.Mail", [], "Mail")}</span>
        {entry.date ? <span style={style.rowDate}>{entry.date}</span> : null}
        {parcel ? <span style={style.rowParcelTag}>{attachmentSummary(t, entry)}</span> : null}
      </span>
      <span style={style.rowActions}>
        <button
          type="button"
          style={miniButtonStyle(canClaim)}
          disabled={!canClaim}
          title={t("client.Read", [], "Read")}
          onClick={(event) => {
            event.stopPropagation();
            if (entry.id !== undefined && canClaim) onClaim?.(entry.id);
          }}
        >
          {t("ui.mailClaimShort", [], "Claim")}
        </button>
        <button
          type="button"
          style={miniButtonStyle(canDelete)}
          disabled={!canDelete}
          title={t("client.Delete", [], "Delete")}
          onClick={(event) => {
            event.stopPropagation();
            if (entry.id !== undefined && canDelete) onDelete?.(entry.id);
          }}
        >
          {t("client.Delete", [], "Delete")}
        </button>
      </span>
    </div>
  );
}

/* ------------------------------------------------------------------------- */
/* Read pane                                                                 */
/* ------------------------------------------------------------------------- */

function MailReadPane({
  t,
  message,
  onClaim,
  onDelete,
  onReply,
  onBack,
}: {
  t: TranslateFn;
  message: MailMessageSummary | null;
  onClaim?: (mailId: number) => void;
  onDelete?: (mailId: number) => void;
  onReply?: () => void;
  onBack: () => void;
}) {
  if (!message) {
    return (
      <>
        <div style={style.readBody}>
          <div style={style.empty}>{t("ui.mailSelectHint", [], "Select a message to read.")}</div>
        </div>
        <div style={style.readActions}>
          <button type="button" style={style.actionButton} onClick={onBack}>
            {t("ui.back", [], "Back")}
          </button>
        </div>
      </>
    );
  }

  const parcel = hasAttachment(message);
  const canClaim = Boolean(onClaim) && parcel && !message.claimed && !message.locked;
  const canDelete = Boolean(onDelete) && message.id !== undefined;

  return (
    <>
      <div style={style.readHeader}>
        <img style={style.readPortrait} src={parcel && !message.items?.length ? ICONS.gold : ICONS.letter} alt="" draggable={false} />
        <div style={style.readHeaderText}>
          <span style={style.readSubject}>{message.subject || t("client.Mail", [], "Mail")}</span>
          <span style={style.readMeta}>
            {`${t("client.Sender", [], "Sender")}: ${senderName(message)}`}
            {message.date ? ` · ${message.date}` : ""}
          </span>
        </div>
      </div>

      <div style={style.readBody}>
        {(message.body ?? "").trim().length ? (
          (message.body ?? "").split(/\r?\n/).map((line, index) => (
            <p key={`mail-body-${index}`} style={style.readParagraph}>
              {line}
            </p>
          ))
        ) : (
          <p style={style.readParagraph}>{t("ui.mailNoBody", [], "(No message body.)")}</p>
        )}
      </div>

      {parcel ? (
        <div style={style.attachBox}>
          <span style={style.attachLabel}>{t("ui.mailAttachment", [], "Attachment")}</span>
          <span style={style.attachValue}>{attachmentSummary(t, message)}</span>
          {message.locked ? (
            <span style={style.attachLocked}>{t("ui.mailLocked", [], "Locked")}</span>
          ) : message.claimed ? (
            <span style={style.attachClaimed}>{t("ui.mailClaimed", [], "Claimed")}</span>
          ) : null}
        </div>
      ) : null}

      <div style={style.readActions}>
        <button type="button" style={style.actionButton} onClick={onBack}>
          {t("ui.back", [], "Back")}
        </button>
        {parcel ? (
          <button
            type="button"
            style={{ ...style.actionButton, ...style.actionButtonPrimary, ...(!canClaim ? style.actionButtonDisabled : null) }}
            disabled={!canClaim}
            onClick={() => message.id !== undefined && canClaim && onClaim?.(message.id)}
          >
            {message.claimed ? t("ui.mailClaimed", [], "Claimed") : t("ui.mailClaim", [], "Claim Attachment")}
          </button>
        ) : null}
        <button
          type="button"
          style={{ ...style.actionButton, ...(!onReply ? style.actionButtonDisabled : null) }}
          disabled={!onReply}
          onClick={() => onReply?.()}
        >
          {t("client.Reply", [], "Reply")}
        </button>
        <button
          type="button"
          style={{ ...style.actionButton, ...(!canDelete ? style.actionButtonDisabled : null) }}
          disabled={!canDelete}
          onClick={() => message.id !== undefined && canDelete && onDelete?.(message.id)}
        >
          {t("client.Delete", [], "Delete")}
        </button>
      </div>
    </>
  );
}

/* ------------------------------------------------------------------------- */
/* Compose form                                                              */
/* ------------------------------------------------------------------------- */

function MailComposeForm({
  t,
  to,
  subject,
  body,
  goldText,
  gold,
  attachableItems,
  selectedItems,
  canSend,
  sendEnabled,
  onToChange,
  onSubjectChange,
  onBodyChange,
  onGoldChange,
  onToggleItem,
  onSubmit,
  onCancel,
}: {
  t: TranslateFn;
  to: string;
  subject: string;
  body: string;
  goldText: string;
  gold?: number;
  attachableItems?: Array<{ key: string; name: string; quantity?: number }>;
  selectedItems: string[];
  canSend: boolean;
  sendEnabled: boolean;
  onToChange: (value: string) => void;
  onSubjectChange: (value: string) => void;
  onBodyChange: (value: string) => void;
  onGoldChange: (value: string) => void;
  onToggleItem: (key: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}) {
  return (
    <form
      style={style.compose}
      onSubmit={(event) => {
        event.preventDefault();
        if (canSend) onSubmit();
      }}
    >
      <label style={style.field}>
        <span style={style.fieldLabel}>{t("ui.mailRecipient", [], "Recipient")}</span>
        <input
          style={style.input}
          value={to}
          maxLength={20}
          autoComplete="off"
          spellCheck={false}
          placeholder={t("client.EnterMailRecipientName", [], "Recipient name")}
          onChange={(event) => onToChange(event.target.value)}
        />
      </label>

      <label style={style.field}>
        <span style={style.fieldLabel}>{t("ui.mailSubject", [], "Subject")}</span>
        <input
          style={style.input}
          value={subject}
          maxLength={40}
          autoComplete="off"
          onChange={(event) => onSubjectChange(event.target.value)}
        />
      </label>

      <label style={style.field}>
        <span style={style.fieldLabel}>{t("client.Message", [], "Message")}</span>
        <textarea
          style={style.textarea}
          value={body}
          rows={5}
          maxLength={300}
          onChange={(event) => onBodyChange(event.target.value)}
        />
      </label>

      <label style={style.field}>
        <span style={style.fieldLabel}>{`${t("client.Gold", [], "Gold")}${typeof gold === "number" ? ` (${formatNumber(gold)})` : ""}`}</span>
        <input
          style={style.input}
          type="number"
          min={0}
          value={goldText}
          placeholder="0"
          onChange={(event) => onGoldChange(event.target.value)}
        />
      </label>

      {attachableItems && attachableItems.length ? (
        <div style={style.field}>
          <span style={style.fieldLabel}>{t("ui.mailAttachItems", [], "Attach Items")}</span>
          <div style={style.attachList}>
            {attachableItems.map((item) => {
              const active = selectedItems.includes(item.key);
              return (
                <button
                  key={item.key}
                  type="button"
                  style={{ ...style.attachChip, ...(active ? style.attachChipOn : null) }}
                  onClick={() => onToggleItem(item.key)}
                >
                  {item.name}
                  {item.quantity && item.quantity > 1 ? ` ×${item.quantity}` : ""}
                </button>
              );
            })}
          </div>
        </div>
      ) : null}

      <div style={style.composeActions}>
        <button
          type="submit"
          style={{ ...style.actionButton, ...style.actionButtonPrimary, ...(!canSend ? style.actionButtonDisabled : null) }}
          disabled={!canSend}
          title={!sendEnabled ? t("ui.mailSendDisabled", [], "Sending is unavailable.") : undefined}
        >
          {t("client.Send", [], "Send")}
        </button>
        <button type="button" style={style.actionButton} onClick={onCancel}>
          {t("ui.cancel", [], "Cancel")}
        </button>
      </div>
    </form>
  );
}

/* ------------------------------------------------------------------------- */
/* Styles                                                                    */
/* ------------------------------------------------------------------------- */

function tabStyle(active: boolean): CSSProperties {
  return {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    borderBottom: active ? "1px solid transparent" : "1px solid rgba(190, 157, 99, 0.5)",
    background: active
      ? "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))"
      : "linear-gradient(180deg, rgba(40, 27, 15, 0.9), rgba(20, 13, 7, 0.9))",
    color: active ? "#f4dcaf" : "#cbb38a",
    padding: "4px 0",
    fontSize: 11,
    fontWeight: active ? 700 : 400,
    cursor: "pointer",
  };
}

function miniButtonStyle(enabled: boolean): CSSProperties {
  return {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#e3d3af",
    padding: "1px 5px",
    fontSize: 9,
    cursor: enabled ? "pointer" : "default",
    opacity: enabled ? 1 : 0.4,
  };
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 356,
    top: 96,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 37,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  title: { position: "absolute", left: 18, top: 9 },
  titleText: {
    position: "absolute",
    left: 18,
    top: 8,
    height: 16,
    lineHeight: "16px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  close: { position: "absolute", left: 288, top: 3 },
  disabled: { opacity: 0.45, cursor: "default" },

  tabs: { position: "absolute", left: 12, top: 34, width: 288, display: "flex", gap: 4 },

  // Inbox list ------------------------------------------------------------
  listHeader: {
    position: "absolute",
    left: 12,
    top: 64,
    width: 288,
    display: "flex",
    fontSize: 10,
    color: "#a89568",
    borderBottom: "1px solid rgba(190, 157, 99, 0.3)",
    paddingBottom: 3,
  },
  colType: { width: 44 },
  colSender: { width: 78 },
  colMessage: { flex: 1 },
  list: {
    position: "absolute",
    left: 12,
    top: 84,
    width: 288,
    height: 300,
    overflowY: "auto",
    display: "flex",
    flexDirection: "column",
    gap: 2,
  },
  empty: { padding: "12px 4px", fontSize: 11, color: "#9c8d6f", textAlign: "center" },
  footer: {
    position: "absolute",
    left: 12,
    top: 408,
    width: 288,
    fontSize: 10,
    color: "#a89568",
    textAlign: "right",
  },
  row: {
    display: "flex",
    alignItems: "center",
    gap: 4,
    padding: "3px 4px",
    border: "1px solid rgba(190, 157, 99, 0.18)",
    background: "rgba(11, 8, 5, 0.45)",
    cursor: "pointer",
  },
  rowSelected: {
    borderColor: "rgba(214, 180, 110, 0.85)",
    background: "rgba(60, 38, 18, 0.7)",
  },
  rowIconArea: { position: "relative", width: 40, height: 22, flex: "0 0 40px" },
  rowIcon: { position: "absolute", left: 0, top: 1, width: 20, height: 20, imageRendering: "pixelated" },
  rowFlag: { position: "absolute", left: 22, top: 0, width: 14, height: 14, imageRendering: "pixelated" },
  rowFlagAlt: { position: "absolute", left: 22, top: 9, width: 14, height: 14, imageRendering: "pixelated" },
  rowSender: { width: 78, fontSize: 11, overflow: "hidden", whiteSpace: "nowrap", textOverflow: "ellipsis", color: "#d6c6a5" },
  rowMessage: { flex: 1, minWidth: 0, display: "flex", flexDirection: "column", fontSize: 11, color: "#cbb38a", overflow: "hidden" },
  rowDate: { fontSize: 9, color: "#8a7c5e" },
  rowParcelTag: { fontSize: 9, color: "#caa64a" },
  rowActions: { display: "flex", flexDirection: "column", gap: 2, flex: "0 0 auto" },
  unreadText: { color: "#f4dcaf", fontWeight: 700 },

  // Read pane -------------------------------------------------------------
  readHeader: {
    position: "absolute",
    left: 12,
    top: 64,
    width: 288,
    display: "flex",
    gap: 8,
    alignItems: "center",
    borderBottom: "1px solid rgba(190, 157, 99, 0.3)",
    paddingBottom: 6,
  },
  readPortrait: { width: 26, height: 26, imageRendering: "pixelated" },
  readHeaderText: { display: "flex", flexDirection: "column", minWidth: 0 },
  readSubject: { fontSize: 12, fontWeight: 700, color: "#f4dcaf", overflow: "hidden", whiteSpace: "nowrap", textOverflow: "ellipsis" },
  readMeta: { fontSize: 10, color: "#a89568" },
  readBody: {
    position: "absolute",
    left: 12,
    top: 104,
    width: 288,
    height: 252,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.22)",
    background: "rgba(11, 8, 5, 0.5)",
    padding: "6px 8px",
    fontSize: 11,
    lineHeight: 1.4,
    color: "#e3d3af",
  },
  readParagraph: { margin: "0 0 6px" },
  attachBox: {
    position: "absolute",
    left: 12,
    top: 362,
    width: 288,
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: "4px 8px",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(27, 19, 10, 0.7)",
    fontSize: 11,
  },
  attachLabel: { color: "#a89568" },
  attachValue: { color: "#f4dcaf", fontWeight: 700, flex: 1 },
  attachLocked: { color: "#d98a8a" },
  attachClaimed: { color: "#8be07a" },

  readActions: { position: "absolute", left: 12, top: 408, width: 288, display: "flex", gap: 6 },

  // Compose form ----------------------------------------------------------
  compose: {
    position: "absolute",
    left: 12,
    top: 64,
    width: 288,
    display: "flex",
    flexDirection: "column",
    gap: 6,
  },
  field: { display: "flex", flexDirection: "column", gap: 2 },
  fieldLabel: { fontSize: 10, color: "#a89568" },
  input: {
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "rgba(19, 12, 8, 0.92)",
    color: "#f4dcaf",
    padding: "4px 6px",
    fontSize: 11,
  },
  textarea: {
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "rgba(19, 12, 8, 0.92)",
    color: "#f4dcaf",
    padding: "4px 6px",
    fontSize: 11,
    resize: "vertical",
    fontFamily: "inherit",
  },
  attachList: { display: "flex", flexWrap: "wrap", gap: 4, maxHeight: 70, overflowY: "auto" },
  attachChip: {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(40, 27, 15, 0.9), rgba(20, 13, 7, 0.9))",
    color: "#cbb38a",
    padding: "2px 6px",
    fontSize: 10,
    cursor: "pointer",
  },
  attachChipOn: {
    borderColor: "rgba(214, 180, 110, 0.85)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
  },
  composeActions: { display: "flex", gap: 6, marginTop: 4 },

  // Shared action buttons -------------------------------------------------
  actionButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "5px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonPrimary: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};

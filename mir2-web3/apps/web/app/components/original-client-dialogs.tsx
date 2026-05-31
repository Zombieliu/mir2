"use client";

import { useEffect, useState } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type MailInventoryItemLike = {
  uniqueId: number;
  name: string;
  icon: number;
  slot: number;
  container: "bag1" | "bag2" | "quest" | "belt" | "storage";
};

type MailAttachment = {
  uniqueId: number;
  name: string;
  icon: number;
};

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
  inventoryItems?: MailInventoryItemLike[];
  onClaim: (mailId: number) => void;
  onDelete: (mailId: number) => void;
  onSendMail: (name: string, message: string, gold: number, itemUniqueIds?: number[]) => void;
  onClose: () => void;
};

const MAIL_ATTACHMENT_SLOTS = 5;

export function MailPanel({ t, mail, inventoryItems = [], onClaim, onDelete, onSendMail, onClose }: MailPanelProps) {
  const entries = mail.filter((message) => !message.deleted);
  const visibleEntries = entries.slice(0, 10);
  const selectedEntry = visibleEntries.find((entry) => entry.id !== undefined) ?? visibleEntries[0] ?? null;
  const pageCount = Math.max(1, Math.ceil(entries.length / 10));

  const [composing, setComposing] = useState(false);
  const [recipient, setRecipient] = useState("");
  const [composeMessage, setComposeMessage] = useState("");
  const [composeGold, setComposeGold] = useState("0");
  const [attachments, setAttachments] = useState<MailAttachment[]>([]);

  const openCompose = (toName: string) => {
    setRecipient(toName);
    setComposeMessage("");
    setComposeGold("0");
    setAttachments([]);
    setComposing(true);
  };

  const attachInventoryItem = (uniqueId: number) => {
    if (attachments.length >= MAIL_ATTACHMENT_SLOTS) return;
    if (attachments.some((entry) => entry.uniqueId === uniqueId)) return;
    const item = inventoryItems.find(
      (entry) => entry.uniqueId === uniqueId && (entry.container === "bag1" || entry.container === "bag2"),
    );
    if (!item) return;
    setAttachments((current) => [...current, { uniqueId, name: item.name, icon: item.icon }]);
  };

  useEffect(() => {
    if (!composing) return;
    function onAttach(event: Event) {
      const detail = (event as CustomEvent<{ uniqueId?: number }>).detail;
      if (detail && typeof detail.uniqueId === "number") attachInventoryItem(detail.uniqueId);
    }
    window.addEventListener("mir2:mailAttach", onAttach);
    return () => window.removeEventListener("mir2:mailAttach", onAttach);
  });

  const removeAttachment = (uniqueId: number) => {
    setAttachments((current) => current.filter((entry) => entry.uniqueId !== uniqueId));
  };

  const submitCompose = () => {
    const name = recipient.trim();
    if (!name) return;
    const gold = Math.max(0, Number.parseInt(composeGold || "0", 10) || 0);
    onSendMail(name, composeMessage, gold, attachments.map((entry) => entry.uniqueId));
    setComposing(false);
  };

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
      <div className="mail-action send"><SpriteButton sprite={ORIGINAL_UI.mail.sendButton} label={t("client.Send", [], "Send")} onClick={() => openCompose("")} /></div>
      <div className="mail-action reply"><SpriteButton sprite={ORIGINAL_UI.mail.replyButton} label={t("client.Reply", [], "Reply")} onClick={() => openCompose(selectedEntry?.from ?? "")} /></div>
      {composing ? (
        <div className="mail-compose" role="dialog" aria-label={t("client.Send", [], "Send")}>
          <div className="mail-compose-title">{t("client.Send", [], "Send Mail")}</div>
          <label className="mail-compose-field">
            <span>{t("client.Sender", [], "To")}</span>
            <input
              type="text"
              value={recipient}
              onChange={(event) => setRecipient(event.target.value)}
              autoFocus
            />
          </label>
          <label className="mail-compose-field message">
            <span>{t("client.Message", [], "Message")}</span>
            <textarea value={composeMessage} onChange={(event) => setComposeMessage(event.target.value)} rows={4} />
          </label>
          <label className="mail-compose-field">
            <span>{t("ui.gold", [], "Gold")}</span>
            <input
              type="number"
              min={0}
              value={composeGold}
              onChange={(event) => setComposeGold(event.target.value)}
            />
          </label>
          <div className="mail-compose-attachments" data-item-drop-kind="mailAttach">
            <span className="mail-compose-attachments-label">{t("ui.attachments", [], "Items")}</span>
            <div className="mail-compose-attachment-slots">
              {Array.from({ length: MAIL_ATTACHMENT_SLOTS }, (_, slot) => {
                const attachment = attachments[slot] ?? null;
                return (
                  <div key={`mail-attach-${slot}`} className="mail-attachment-slot">
                    {attachment ? (
                      <button
                        type="button"
                        className="mail-attachment-item"
                        title={attachment.name}
                        aria-label={attachment.name}
                        onClick={() => removeAttachment(attachment.uniqueId)}
                      >
                        <img src={`/original-ui/Items/${attachment.icon}.png`} alt="" draggable={false} />
                      </button>
                    ) : null}
                  </div>
                );
              })}
            </div>
          </div>
          <div className="mail-compose-actions">
            <button type="button" className="mail-compose-send" disabled={!recipient.trim()} onClick={submitCompose}>
              {t("client.Send", [], "Send")}
            </button>
            <button type="button" className="mail-compose-cancel" onClick={() => setComposing(false)}>
              {t("ui.cancel", [], "Cancel")}
            </button>
          </div>
        </div>
      ) : null}
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
};

export function NpcDialogPanel({
  t,
  dialog,
  onClose,
  onSelectTarget,
  onSubmitInput,
}: NpcDialogPanelProps) {
  const [inputValue, setInputValue] = useState("");
  const bodyLines = dialog.body.map(stripCrystalDialogMarkup).filter((line) => line.trim().length > 0);
  const title = stripCrystalDialogMarkup(dialog.title || dialog.npcName);
  const footer = stripCrystalDialogMarkup(dialog.footer);

  return (
    <section className="npc-dialog-panel">
      <img
        className="npc-dialog-frame"
        src={ORIGINAL_UI.npc.frame}
        alt=""
        draggable={false}
        onError={(event) => {
          // Until Prguse/995 is exported, hide the broken frame and let the
          // CSS fallback chrome stand in.
          const img = event.currentTarget;
          img.style.display = "none";
          img.parentElement?.classList.add("npc-dialog-frame-fallback");
        }}
      />
      <strong className="npc-dialog-title">{title}</strong>
      <div className="npc-dialog-help">
        <SpriteButton sprite={ORIGINAL_UI.npc.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
      </div>
      <div className="npc-dialog-close">
        <SpriteButton sprite={ORIGINAL_UI.npc.closeButton} label={t("ui.close")} onClick={onClose} />
      </div>
      <div className="npc-dialog-body">
        {bodyLines.map((line, index) => (
          <p key={`${dialog.npcObjectId}-${index}`}>{line}</p>
        ))}
        {dialog.links.length ? (
          <div className="npc-dialog-links">
            {dialog.links.map((link, index) => (
              <button
                key={`${dialog.npcObjectId}-link-${index}-${link.target}`}
                type="button"
                data-target={link.target}
                onClick={() => onSelectTarget(link.target)}
              >
                {stripCrystalDialogMarkup(link.text)}
              </button>
            ))}
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

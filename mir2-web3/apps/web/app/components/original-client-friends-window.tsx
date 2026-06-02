"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A single friend / blocked entry. */
export type FriendEntry = {
  name: string;
  online?: boolean;
  memo?: string;
};

/**
 * Mirrors the stage-5 `social` slice (`{ friends?: string[]; blocked?: string[] }`)
 * but accepts richer entries (with online + memo) when the host can provide them.
 */
export type FriendsSummary = {
  friends?: Array<string | FriendEntry>;
  blocked?: Array<string | FriendEntry>;
};

export type FriendsWindowProps = {
  t: TranslateFn;
  social: FriendsSummary | null;
  onAddFriend?: (name: string) => void;
  onRemoveFriend?: (name: string) => void;
  onBlockPlayer?: (name: string) => void;
  onUnblockPlayer?: (name: string) => void;
  /** Whisper / private message to the selected friend. */
  onWhisper?: (name: string) => void;
  onClose: () => void;
};

type FriendsTab = "friends" | "blocked";

const FRAME = ORIGINAL_UI.mail;

const ROWS_PER_PAGE = 10;

export function FriendsWindow({
  t,
  social,
  onAddFriend,
  onRemoveFriend,
  onBlockPlayer,
  onUnblockPlayer,
  onWhisper,
  onClose,
}: FriendsWindowProps) {
  const [tab, setTab] = useState<FriendsTab>("friends");
  const [page, setPage] = useState(0);
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [addName, setAddName] = useState("");

  const friends = useMemo(() => normalizeEntries(social?.friends), [social?.friends]);
  const blocked = useMemo(() => normalizeEntries(social?.blocked), [social?.blocked]);
  const rows = tab === "friends" ? friends : blocked;
  const onlineCount = friends.filter((entry) => entry.online).length;

  const pageCount = Math.max(1, Math.ceil(rows.length / ROWS_PER_PAGE));
  const currentPage = Math.min(page, pageCount - 1);
  const visible = rows.slice(currentPage * ROWS_PER_PAGE, currentPage * ROWS_PER_PAGE + ROWS_PER_PAGE);

  const selected = useMemo(() => {
    if (selectedName) {
      const match = rows.find((entry) => entry.name === selectedName);
      if (match) return match;
    }
    return visible[0] ?? rows[0] ?? null;
  }, [rows, selectedName, visible]);

  useEffect(() => {
    setPage(0);
  }, [tab]);

  useEffect(() => {
    if (page > pageCount - 1) {
      setPage(pageCount - 1);
    }
  }, [page, pageCount]);

  return (
    <section
      aria-label={t("ui.friend", [], "Friends")}
      data-friends-tab={tab}
      data-friends-selected={selected?.name ?? ""}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.title} src={FRAME.title} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.friend", [], "Friends")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>
      <div style={style.help}>
        <SpriteButton sprite={FRAME.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.friend", [], "Friends")}>
        <button
          type="button"
          role="tab"
          data-friends-tab="friends"
          aria-selected={tab === "friends"}
          onClick={() => setTab("friends")}
          style={{ ...style.tab, ...(tab === "friends" ? style.tabActive : null) }}
        >
          {t("ui.friendList", [], "Friends")}
          <span style={style.tabCount}>{`${onlineCount}/${friends.length}`}</span>
        </button>
        <button
          type="button"
          role="tab"
          data-friends-tab="blocked"
          aria-selected={tab === "blocked"}
          onClick={() => setTab("blocked")}
          style={{ ...style.tab, ...(tab === "blocked" ? style.tabActive : null) }}
        >
          {t("ui.friendBlocked", [], "Blocked")}
          <span style={style.tabCount}>{blocked.length}</span>
        </button>
      </div>

      <div style={style.list} aria-label={tab === "friends" ? t("ui.friendList", [], "Friends") : t("ui.friendBlocked", [], "Blocked")}>
        {visible.length === 0 ? (
          <div style={style.empty}>
            {tab === "friends"
              ? t("ui.friendEmpty", [], "Your friend list is empty.")
              : t("ui.friendBlockedEmpty", [], "You have not blocked anyone.")}
          </div>
        ) : (
          visible.map((entry) => {
            const isSelected = selected?.name === entry.name;
            return (
              <button
                key={entry.name}
                type="button"
                data-friend-name={entry.name}
                aria-pressed={isSelected}
                onClick={() => setSelectedName(entry.name)}
                style={{ ...style.row, ...(isSelected ? style.rowSelected : null) }}
              >
                {tab === "friends" ? (
                  <span
                    style={{ ...style.statusDot, background: entry.online ? "#8be07a" : "#5c5648" }}
                    aria-hidden="true"
                  />
                ) : (
                  <span style={style.blockDot} aria-hidden="true" />
                )}
                <span style={style.rowName}>{entry.name}</span>
                {tab === "friends" ? (
                  <span style={{ ...style.rowState, color: entry.online ? "#8be07a" : "#9c8d6f" }}>
                    {entry.online ? t("ui.guildStateOnline", [], "Online") : t("ui.guildStateOffline", [], "Offline")}
                  </span>
                ) : null}
              </button>
            );
          })
        )}
      </div>

      <div style={style.pagePrev}>
        <SpriteButton
          sprite={FRAME.previousButton}
          label={t("ui.previous", [], "Previous")}
          onClick={() => setPage((current) => Math.max(0, current - 1))}
        />
      </div>
      <div style={style.pageLabel}>{`${currentPage + 1} / ${pageCount}`}</div>
      <div style={style.pageNext}>
        <SpriteButton
          sprite={FRAME.nextButton}
          label={t("ui.next", [], "Next")}
          onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))}
        />
      </div>

      <div style={style.detail} data-friend-detail={selected?.name ?? ""}>
        {selected ? (
          <>
            <div style={style.detailName}>{selected.name}</div>
            <div style={style.detailMeta}>
              {tab === "friends"
                ? selected.online
                  ? t("ui.guildStateOnline", [], "Online")
                  : t("ui.guildStateOffline", [], "Offline")
                : t("ui.friendBlocked", [], "Blocked")}
            </div>
            {selected.memo ? <p style={style.memo}>{selected.memo}</p> : null}
          </>
        ) : (
          <div style={style.empty}>{t("ui.friendSelectHint", [], "Select a name to view details.")}</div>
        )}
      </div>

      <div style={style.actions}>
        {tab === "friends" ? (
          <>
            <button
              type="button"
              disabled={!selected || !onWhisper}
              style={{ ...style.actionButton, ...(!selected || !onWhisper ? style.actionButtonDisabled : null) }}
              onClick={() => selected && onWhisper?.(selected.name)}
            >
              {t("ui.friendWhisper", [], "Whisper")}
            </button>
            <button
              type="button"
              disabled={!selected || !onRemoveFriend}
              style={{ ...style.actionButton, ...(!selected || !onRemoveFriend ? style.actionButtonDisabled : null) }}
              onClick={() => selected && onRemoveFriend?.(selected.name)}
            >
              {t("ui.friendRemove", [], "Remove")}
            </button>
          </>
        ) : (
          <button
            type="button"
            disabled={!selected || !onUnblockPlayer}
            style={{ ...style.actionButton, flex: 1, ...(!selected || !onUnblockPlayer ? style.actionButtonDisabled : null) }}
            onClick={() => selected && onUnblockPlayer?.(selected.name)}
          >
            {t("ui.friendUnblock", [], "Unblock")}
          </button>
        )}
      </div>

      <form
        style={style.addRow}
        onSubmit={(event) => {
          event.preventDefault();
          const trimmed = addName.trim();
          if (!trimmed) return;
          if (tab === "friends") {
            onAddFriend?.(trimmed);
          } else {
            onBlockPlayer?.(trimmed);
          }
          setAddName("");
        }}
      >
        <input
          style={style.input}
          value={addName}
          onChange={(event) => setAddName(event.target.value)}
          placeholder={t("ui.friendAddPlaceholder", [], "Player name")}
          aria-label={tab === "friends" ? t("ui.friendAdd", [], "Add Friend") : t("ui.friendBlock", [], "Block")}
          autoComplete="off"
          spellCheck={false}
        />
        <button
          type="submit"
          disabled={
            addName.trim().length === 0 ||
            (tab === "friends" ? !onAddFriend : !onBlockPlayer)
          }
          style={{
            ...style.actionButton,
            flex: "0 0 90px",
            ...(addName.trim().length === 0 || (tab === "friends" ? !onAddFriend : !onBlockPlayer)
              ? style.actionButtonDisabled
              : null),
          }}
        >
          {tab === "friends" ? t("ui.friendAdd", [], "Add") : t("ui.friendBlock", [], "Block")}
        </button>
      </form>
    </section>
  );
}

function normalizeEntries(entries?: Array<string | FriendEntry>): FriendEntry[] {
  if (!entries) return [];
  return entries.map((entry) => (typeof entry === "string" ? { name: entry } : entry));
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 250,
    top: 5,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 29,
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
  help: { position: "absolute", left: 262, top: 3 },
  tabs: { position: "absolute", left: 10, top: 30, width: 292, display: "flex", gap: 3 },
  tab: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "3px 0",
    fontSize: 11,
    cursor: "pointer",
    display: "flex",
    justifyContent: "center",
    alignItems: "center",
    gap: 5,
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  tabCount: { fontSize: 9, opacity: 0.85 },
  list: {
    position: "absolute",
    left: 10,
    top: 56,
    width: 292,
    height: 224,
    display: "flex",
    flexDirection: "column",
    gap: 2,
    overflow: "hidden",
  },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  row: {
    display: "flex",
    alignItems: "center",
    gap: 7,
    width: "100%",
    height: 22,
    padding: "0 7px",
    border: "1px solid transparent",
    background: "rgba(20, 13, 7, 0.4)",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
  },
  rowSelected: { background: "rgba(95, 53, 24, 0.5)", borderColor: "rgba(214, 180, 110, 0.7)" },
  statusDot: { width: 8, height: 8, borderRadius: "50%", flex: "0 0 auto" },
  blockDot: {
    width: 8,
    height: 8,
    borderRadius: "50%",
    flex: "0 0 auto",
    background: "transparent",
    border: "1px solid #b5563a",
  },
  rowName: { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  rowState: { fontSize: 10, flex: "0 0 auto" },
  pagePrev: { position: "absolute", left: 132, top: 284 },
  pageLabel: {
    position: "absolute",
    left: 150,
    top: 284,
    width: 64,
    textAlign: "center",
    fontSize: 11,
    color: "#cbb38a",
    lineHeight: "16px",
  },
  pageNext: { position: "absolute", left: 214, top: 284 },
  detail: {
    position: "absolute",
    left: 12,
    top: 306,
    width: 288,
    height: 60,
    overflow: "hidden",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
  },
  detailName: { color: "#f8e6bb", fontSize: 12, fontWeight: 700, marginBottom: 2 },
  detailMeta: { fontSize: 10, color: "#cbb38a" },
  memo: { margin: "4px 0 0", fontSize: 11, color: "#d6c6a5", lineHeight: 1.3 },
  actions: { position: "absolute", left: 12, top: 376, width: 288, display: "flex", gap: 6 },
  addRow: { position: "absolute", left: 12, top: 408, width: 288, display: "flex", gap: 6 },
  input: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "4px 8px",
    fontSize: 11,
  },
  actionButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "4px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};

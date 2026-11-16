"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";
import { r16GroupMemberRect, R16_SOCIAL_LAYOUT } from "../../lib/r16-social-layout";
import { R16SocialButton, R16SocialModal, R16_SOCIAL_ART_STYLE, useR16SocialWindow } from "./r16-social-controls";

type TranslateFn = (key: string, params?: Array<string | number>, fallback?: string) => string;

export type GroupMember = {
  name: string;
  leader?: boolean;
  online?: boolean;
  level?: number;
  classKey?: "warrior" | "wizard" | "taoist" | "assassin" | "archer";
  hp?: number;
  maxHp?: number;
};

export type GroupSummary = {
  members?: Array<string | GroupMember>;
  lootMode?: string;
  allowInvites?: boolean;
  maxMembers?: number;
};

export type GroupWindowProps = {
  t: TranslateFn;
  group: GroupSummary | null;
  playerName?: string | null;
  onInviteMember?: (name: string) => boolean | void;
  onKickMember?: (name: string) => boolean | void;
  onLeaveGroup?: () => void;
  onToggleLootMode?: () => void;
  onToggleAllowInvites?: (allow: boolean) => void;
  onPromoteLeader?: (name: string) => void;
  incomingInvite?: Readonly<{ name: string; epoch: number }> | null;
  onReplyInvite?: (epoch: number, acceptInvite: boolean) => void;
  onClose: () => void;
};

const layout = R16_SOCIAL_LAYOUT.group;
const invitation = R16_SOCIAL_LAYOUT.invitation;

export function GroupWindow({ t, group, playerName, onInviteMember, onKickMember, onToggleAllowInvites,
  incomingInvite, onReplyInvite, onClose }: GroupWindowProps) {
  const window = useR16SocialWindow(layout.panel);
  const members = useMemo(() => (group?.members ?? []).map((member, index) =>
    typeof member === "string" ? { name: member, leader: index === 0 } : member), [group?.members]);
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [editor, setEditor] = useState<{ remove: boolean; draft: string } | null>(null);
  const leader = members.find(member => member.leader) ?? members[0];
  const canManage = members.length === 0 || Boolean(playerName && leader?.name === playerName);
  const groupFull = members.length >= (group?.maxMembers && group.maxMembers > 0 ? group.maxMembers : 15);
  const allowInvites = group?.allowInvites ?? true;
  useEffect(() => { if (selectedName && !members.some(member => member.name === selectedName)) setSelectedName(null); }, [members, selectedName]);
  useEffect(() => { if (incomingInvite) setEditor(null); }, [incomingInvite?.name, incomingInvite?.epoch]);
  const openEditor = (remove: boolean) => {
    if (!canManage || (!remove && groupFull) || (remove ? !onKickMember : !onInviteMember)) return;
    setEditor({ remove, draft: "" });
  };
  const submitName = () => {
    if (!editor || !canManage || (!editor.remove && groupFull)) return;
    const name = editor.draft.trim();
    const submit = editor.remove ? onKickMember : onInviteMember;
    if (!name || !submit) return;
    if (submit(name) === false) return;
    setEditor(null);
  };
  const reply = (accept: boolean) => {
    if (incomingInvite && onReplyInvite) onReplyInvite(incomingInvite.epoch, accept);
  };
  return <>
    <section aria-label={t("ui.group", [], "Party")} data-group-size={members.length}
      data-group-loot={(group?.lootMode ?? "").toLowerCase() === "group" ? "group" : "solo"}
      data-r16-group-window="true" data-ui-interactive="true"
      onPointerDown={window.pointerDown} onPointerMove={window.pointerMove} onPointerUp={window.pointerUp} onLostPointerCapture={window.pointerUp}
      style={{ ...style.window, ...layout.panel, ...window.position }}>
      <img src="/original-ui/Prguse/120.png" alt="" draggable={false} style={R16_SOCIAL_ART_STYLE} />
      <img src="/original-ui/Title/5.png" alt="" draggable={false} style={{ position: "absolute", ...layout.title, pointerEvents: "none" }} />
      <R16SocialButton library="Prguse2" frame={360} rect={layout.close} label={t("ui.close", [], "Close")} onClick={onClose} />
      {members.slice(0, 15).map((member, index) => <button key={member.name} type="button" data-member-name={member.name}
        aria-pressed={selectedName === member.name} onClick={() => setSelectedName(member.name)}
        title={member.name} style={{ ...style.member, ...r16GroupMemberRect(index)!, ...(selectedName === member.name ? { outline: "1px solid #c8a660" } : null) }}>
        {member.name}
      </button>)}
      <R16SocialButton library="Prguse" frame={allowInvites ? 117 : 114} rect={layout.allow}
        label={allowInvites ? t("ui.groupAllowOn", [], "Invites On") : t("ui.groupAllowOff", [], "Invites Off")}
        aria-pressed={allowInvites} disabled={!onToggleAllowInvites} onClick={() => onToggleAllowInvites?.(!allowInvites)} />
      {canManage ? <>
        <R16SocialButton frame={members.length ? 133 : 130} rect={layout.add} data-r16-group-add="true"
          label={t("ui.groupInvite", [], "Invite")} disabled={!onInviteMember || groupFull} onClick={() => openEditor(false)} />
        <R16SocialButton frame={136} rect={layout.remove} data-r16-group-remove="true"
          label={t("ui.groupKick", [], "Remove")} disabled={!onKickMember} onClick={() => openEditor(true)} />
      </> : null}
    </section>
    {incomingInvite ? <R16SocialModal label={t("ui.group", [], "Party")} rect={invitation.panel}
      background="/original-ui/Prguse/360.png" onCancel={() => reply(false)} onConfirm={() => reply(true)}>
      <div style={style.invitationText}>{t("ui.groupInviteFrom", [incomingInvite.name], "Do you want to group with " + incomingInvite.name + "?")}</div>
      <R16SocialButton frame={206} rect={invitation.accept} data-r16-group-invite-accept={incomingInvite.epoch}
        label={t("ui.accept", [], "Accept")} disabled={!onReplyInvite} onClick={() => reply(true)} />
      <R16SocialButton frame={210} rect={invitation.decline} data-r16-group-invite-decline={incomingInvite.epoch}
        label={t("ui.decline", [], "Decline")} disabled={!onReplyInvite} onClick={() => reply(false)} />
    </R16SocialModal> : editor ? <R16SocialModal label={editor.remove ? t("ui.groupKick", [], "Remove") : t("ui.groupInvite", [], "Invite")}
      rect={layout.input} background="/original-ui/Prguse/660.png" onCancel={() => setEditor(null)} onConfirm={submitName}>
      <div style={style.inputPrompt}>{editor.remove
        ? t("ui.groupRemoveNamePrompt", [], "Please enter the name of the person you wish to remove.")
        : t("ui.groupAddNamePrompt", [], "Please enter the name of the person you wish to add.")}</div>
      <input data-r16-group-name="true" data-r16-autofocus="true" aria-label={t("ui.playerName", [], "Player name")}
        value={editor.draft} onChange={event => setEditor({ ...editor, draft: event.target.value })} autoComplete="off" spellCheck={false}
        style={{ ...style.input, ...layout.inputField }} />
      <R16SocialButton frame={200} rect={layout.inputConfirm} data-r16-group-name-submit="true"
        label={t("ui.confirm", [], "Confirm")} disabled={!editor.draft.trim()} onClick={submitName} />
      <R16SocialButton frame={203} rect={layout.inputCancel} label={t("ui.cancel", [], "Cancel")} onClick={() => setEditor(null)} />
    </R16SocialModal> : null}
  </>;
}

const style: Record<string, CSSProperties> = {
  window: { position: "absolute", zIndex: 30, color: "#fff", fontSize: 32 / 3,
    fontFamily: "Arial, Helvetica, sans-serif", textShadow: "1px 1px #000", pointerEvents: "auto", touchAction: "none" },
  member: { position: "absolute", padding: 0, border: 0, color: "#fff", background: "transparent", font: "inherit",
    textShadow: "inherit", textAlign: "left", overflow: "hidden", whiteSpace: "nowrap", cursor: "pointer" },
  invitationText: { position: "absolute", left: 35, top: 35, width: 390, height: 110, overflow: "auto", overflowWrap: "anywhere", whiteSpace: "pre-wrap" },
  inputPrompt: { position: "absolute", left: 25, top: 25, width: 235, height: 40, overflowWrap: "anywhere" },
  input: { position: "absolute", boxSizing: "border-box", padding: "0 2px", border: "1px solid #c7a657", background: "#000", color: "#fff", font: "inherit" },
};

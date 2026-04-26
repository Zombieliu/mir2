"use client";

import { useState, useTransition } from "react";

type SubmitState =
  | { kind: "idle"; message: string }
  | { kind: "ok"; message: string }
  | { kind: "error"; message: string };

type MailCommandFormText = {
  idle: string;
  submitting: string;
  rejected: string;
  queued: string;
  targetKind: string;
  targetCharacter: string;
  targetAccount: string;
  targetGlobal: string;
  targetId: string;
  subject: string;
  defaultSubject: string;
  attachment: string;
  body: string;
  defaultBody: string;
  reason: string;
  defaultReason: string;
  queueing: string;
  queue: string;
  preview: string;
};

export function MailCommandForm({ text }: { text: MailCommandFormText }) {
  const [state, setState] = useState<SubmitState>({
    kind: "idle",
    message: text.idle
  });
  const [isPending, startTransition] = useTransition();

  function submit(formData: FormData) {
    startTransition(async () => {
      setState({ kind: "idle", message: text.submitting });
      const response = await fetch("/api/admin/system-mail", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          targetKind: formData.get("targetKind"),
          targetId: formData.get("targetId"),
          subject: formData.get("subject"),
          body: formData.get("body"),
          reason: formData.get("reason"),
          attachments: [
            {
              itemId: String(formData.get("itemId") || "gold"),
              count: Number(formData.get("count") || 1)
            }
          ]
        })
      });
      const payload = (await response.json()) as { error?: string; commandId?: string };
      if (!response.ok) {
        setState({
          kind: "error",
          message: payload.error ?? text.rejected.replace("{status}", String(response.status))
        });
        return;
      }
      setState({
        kind: "ok",
        message: text.queued.replace("{id}", payload.commandId ?? "unknown")
      });
    });
  }

  return (
    <form action={submit} className="form-grid">
      <div className="field">
        <label>{text.targetKind}</label>
        <select className="control" defaultValue="character" name="targetKind">
          <option value="character">{text.targetCharacter}</option>
          <option value="account">{text.targetAccount}</option>
          <option value="global">{text.targetGlobal}</option>
        </select>
      </div>
      <div className="field">
        <label>{text.targetId}</label>
        <input className="control" defaultValue="Scout" name="targetId" />
      </div>
      <div className="field">
        <label>{text.subject}</label>
        <input className="control" defaultValue={text.defaultSubject} name="subject" />
      </div>
      <div className="field">
        <label>{text.attachment}</label>
        <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 90px" }}>
          <input className="control" defaultValue="gold" name="itemId" />
          <input className="control" defaultValue="5000" min="1" name="count" type="number" />
        </div>
      </div>
      <div className="field full">
        <label>{text.body}</label>
        <textarea
          className="control"
          defaultValue={text.defaultBody}
          name="body"
        />
      </div>
      <div className="field full">
        <label>{text.reason}</label>
        <input
          className="control"
          defaultValue={text.defaultReason}
          name="reason"
        />
      </div>
      <div className="field full">
        <div className="actions">
          <button className="button" disabled={isPending} type="submit">
            {isPending ? text.queueing : text.queue}
          </button>
          <button className="button secondary" type="button">
            {text.preview}
          </button>
        </div>
      </div>
      <div className={`notice ${state.kind === "error" ? "badge danger" : ""}`}>
        {state.message}
      </div>
    </form>
  );
}

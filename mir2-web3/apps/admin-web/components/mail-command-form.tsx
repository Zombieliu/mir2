"use client";

import { useState, useTransition } from "react";

type SubmitState =
  | { kind: "idle"; message: string }
  | { kind: "ok"; message: string }
  | { kind: "error"; message: string };

export function MailCommandForm() {
  const [state, setState] = useState<SubmitState>({
    kind: "idle",
    message: "Submits to Rust Admin API with operator headers from server env."
  });
  const [isPending, startTransition] = useTransition();

  function submit(formData: FormData) {
    startTransition(async () => {
      setState({ kind: "idle", message: "Submitting command..." });
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
          message: payload.error ?? `Command rejected with ${response.status}`
        });
        return;
      }
      setState({
        kind: "ok",
        message: `Queued command ${payload.commandId ?? "unknown"}`
      });
    });
  }

  return (
    <form action={submit} className="form-grid">
      <div className="field">
        <label>Target Kind</label>
        <select className="control" defaultValue="character" name="targetKind">
          <option value="character">Character</option>
          <option value="account">Account</option>
          <option value="global">Global</option>
        </select>
      </div>
      <div className="field">
        <label>Target ID</label>
        <input className="control" defaultValue="Scout" name="targetId" />
      </div>
      <div className="field">
        <label>Subject</label>
        <input className="control" defaultValue="Compensation Package" name="subject" />
      </div>
      <div className="field">
        <label>Attachment</label>
        <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 90px" }}>
          <input className="control" defaultValue="gold" name="itemId" />
          <input className="control" defaultValue="5000" min="1" name="count" type="number" />
        </div>
      </div>
      <div className="field full">
        <label>Body</label>
        <textarea
          className="control"
          defaultValue="This mail is delivered through audited Admin API into the live gateway mail store."
          name="body"
        />
      </div>
      <div className="field full">
        <label>Required Reason</label>
        <input
          className="control"
          defaultValue="local GM live mail integration smoke"
          name="reason"
        />
      </div>
      <div className="field full">
        <div className="actions">
          <button className="button" disabled={isPending} type="submit">
            {isPending ? "Queueing..." : "Queue System Mail"}
          </button>
          <button className="button secondary" type="button">
            Preview Impact
          </button>
        </div>
      </div>
      <div className={`notice ${state.kind === "error" ? "badge danger" : ""}`}>
        {state.message}
      </div>
    </form>
  );
}

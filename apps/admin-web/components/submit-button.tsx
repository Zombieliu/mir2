"use client";

import { useFormStatus } from "react-dom";

export function SubmitButton({
  idle,
  pending,
  className = "button",
  disabled = false,
  confirmMessage
}: {
  idle: string;
  pending: string;
  className?: string;
  disabled?: boolean;
  confirmMessage?: string;
}) {
  const status = useFormStatus();
  const busy = status.pending;

  return (
    <button
      aria-busy={busy}
      className={className}
      data-pending={busy ? "true" : "false"}
      disabled={disabled || busy}
      onClick={(event) => {
        if (confirmMessage && !window.confirm(confirmMessage)) {
          event.preventDefault();
        }
      }}
      type="submit"
    >
      {busy ? <span aria-hidden="true" className="button-spinner" /> : null}
      <span aria-live="polite">{busy ? pending : idle}</span>
    </button>
  );
}

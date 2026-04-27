"use client";

import { useFormStatus } from "react-dom";

export function SubmitButton({
  idle,
  pending,
  className = "button"
}: {
  idle: string;
  pending: string;
  className?: string;
}) {
  const status = useFormStatus();

  return (
    <button className={className} disabled={status.pending} type="submit">
      {status.pending ? pending : idle}
    </button>
  );
}

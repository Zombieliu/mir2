"use client";

import { useState } from "react";
import type { ProductCopy } from "@/lib/product-copy";
import { getCopy, isLocale, locales } from "@/lib/site-copy";

type SubmitState = "idle" | "submitting" | "stored" | "preview" | "error";

function readAttribution() {
  try {
    const value = window.localStorage.getItem("numeron.attribution.v1");
    return value ? JSON.parse(value) as unknown : null;
  } catch {
    return null;
  }
}

export function FounderInterestForm({ copy, locale }: { copy: ProductCopy["membership"]; locale: string }) {
  const [state, setState] = useState<SubmitState>("idle");

  async function submit(formData: FormData) {
    setState("submitting");
    const response = await fetch("/api/founder-interest", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        email: formData.get("email"),
        region: formData.get("region"),
        plan: formData.get("plan"),
        consent: formData.get("consent") === "on",
        company: formData.get("company"),
        locale,
        attribution: readAttribution(),
      }),
    }).catch(() => null);

    if (!response?.ok) {
      setState("error");
      return;
    }
    const result = await response.json() as { mode?: "stored" | "preview" };
    setState(result.mode === "stored" ? "stored" : "preview");
  }

  return (
    <form className="founder-form" action={submit}>
      <label><span>{copy.email}</span><input required type="email" name="email" autoComplete="email" placeholder="name@example.com" /></label>
      <label><span>{copy.region}</span><select name="region" defaultValue={isLocale(locale) ? locale : "en"}>{locales.map((item) => <option value={item} key={item}>{getCopy(item).languageName}</option>)}</select></label>
      <label><span>{copy.plan}</span><select name="plan" defaultValue="token"><option value="token">Rebirth Token</option><option value="architect">{getCopy(isLocale(locale) ? locale : "en").membership.plans[2].name}</option><option value="free">{getCopy(isLocale(locale) ? locale : "en").membership.plans[0].name}</option></select></label>
      <label className="founder-honeypot" aria-hidden="true"><span>{copy.company}</span><input name="company" tabIndex={-1} autoComplete="off" /></label>
      <label className="founder-consent"><input required type="checkbox" name="consent" /><span>{copy.consent}</span></label>
      <button className="button button-primary" type="submit" disabled={state === "submitting"} data-track="founder_interest_submit"><span>{state === "submitting" ? "…" : copy.submit}</span><i>↗</i></button>
      {state === "stored" ? <p className="form-result success">{copy.submitted}</p> : null}
      {state === "preview" ? <p className="form-result preview">{copy.previewSubmitted}</p> : null}
      {state === "error" ? <p className="form-result error">{copy.submitError}</p> : null}
    </form>
  );
}

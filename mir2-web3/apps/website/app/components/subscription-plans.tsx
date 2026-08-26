"use client";

import { useState } from "react";
import type { SiteCopy } from "@/lib/site-copy";

type SubscriptionPlansProps = {
  copy: SiteCopy["membership"];
  checkoutUrl?: string;
};

function checkoutHref(baseUrl: string, plan: string, billing: "monthly" | "annual") {
  const separator = baseUrl.includes("?") ? "&" : "?";
  return `${baseUrl}${separator}plan=${encodeURIComponent(plan)}&billing=${billing}`;
}

export function SubscriptionPlans({ copy, checkoutUrl }: SubscriptionPlansProps) {
  const [billing, setBilling] = useState<"monthly" | "annual">("monthly");

  return (
    <div className="subscription-shell">
      <div className="billing-control" role="group" aria-label="Billing period">
        <button type="button" aria-pressed={billing === "monthly"} onClick={() => setBilling("monthly")}>{copy.monthly}</button>
        <button type="button" aria-pressed={billing === "annual"} onClick={() => setBilling("annual")}>{copy.annual}<small>{copy.annualNote}</small></button>
      </div>

      <p className="prototype-price"><span />{copy.prototypePrice}</p>

      <div className="plan-grid">
        {copy.plans.map((plan, index) => {
          const recommended = plan.id === "token";
          const price = billing === "monthly" ? plan.monthlyPrice : plan.annualPrice;
          const unit = plan.id === "free" ? null : billing === "monthly" ? copy.perMonth : copy.perYear;
          return (
            <article className={`plan-card${recommended ? " plan-card-featured" : ""}`} key={plan.id}>
              <div className="plan-card-topline"><span>/{String(index + 1).padStart(2, "0")}</span>{recommended ? <b>{copy.recommended}</b> : null}</div>
              <h2>{plan.name}</h2>
              <p>{plan.tagline}</p>
              <div className="plan-price"><strong>{price}</strong>{unit ? <span>{unit}</span> : null}</div>
              {checkoutUrl && plan.id !== "free" ? (
                <a className="plan-action" href={checkoutHref(checkoutUrl, plan.id, billing)}>{plan.name}<span>↗</span></a>
              ) : (
                <button className="plan-action" type="button" disabled>{plan.id === "free" ? copy.plans[0].monthlyPrice : copy.unavailable}<span>↗</span></button>
              )}
              <ul>
                {plan.features.map((feature) => <li key={feature}><i>✓</i>{feature}</li>)}
              </ul>
            </article>
          );
        })}
      </div>
    </div>
  );
}

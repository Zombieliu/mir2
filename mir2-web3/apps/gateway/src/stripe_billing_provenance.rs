//! Provider evidence extraction. Called only after the parent re-queries and
//! binds a signed Stripe event to its immutable game order.
use super::*;
use mir2_simulation::billing_provenance::{VerifiedPaymentAdverseEvent, VerifiedPaymentRevenue};

pub(super) fn payment(
    session: &Value,
    order: &RechargeOrder,
    event_id: &str,
    occurred: u64,
    now: u64,
) -> Result<VerifiedPaymentRevenue, String> {
    if occurred.saturating_add(999) < order.created_at_ms {
        return Err("billingRevenueEvidenceMismatch".into());
    }
    let occurred = occurred.max(order.created_at_ms);
    let tax = session["total_details"]["amount_tax"].as_u64();
    if tax.is_some_and(|tax| tax > order.offer.amount_minor)
        || occurred < order.created_at_ms
        || occurred > now
    {
        return Err("billingRevenueEvidenceMismatch".into());
    }
    Ok(VerifiedPaymentRevenue {
        provider: "stripe".into(),
        source_event_id: event_id.into(),
        order_id: order.id.clone(),
        checkout_session_id: field(session, "id")?.into(),
        payment_intent_id: field(session, "payment_intent")?.into(),
        livemode: order.livemode,
        currency: order.offer.currency.clone(),
        gross_amount_minor: order.offer.amount_minor,
        indirect_tax_minor: tax,
        discount_minor: session["total_details"]["amount_discount"].as_u64(),
        paid_at_ms: occurred,
        observed_at_ms: now,
        evidence_ref: format!(
            "stripe:checkout.session:{}:event:{event_id}",
            field(session, "id")?
        ),
    })
}

pub(super) fn adverse(
    charge: &Value,
    dispute: Option<&Value>,
    order: &RechargeOrder,
    event_id: &str,
    occurred: u64,
    now: u64,
) -> Result<VerifiedPaymentAdverseEvent, String> {
    if occurred.saturating_add(999) < order.created_at_ms {
        return Err("billingRevenueAdverseEvidenceMismatch".into());
    }
    let occurred = occurred.max(order.created_at_ms);
    let refunded = charge["amount_refunded"]
        .as_u64()
        .ok_or("billingRevenueAdverseEvidenceMismatch")?;
    let (amount, status) = if let Some(dispute) = dispute {
        if dispute["object"].as_str() != Some("dispute")
            || dispute["charge"].as_str() != charge["id"].as_str()
            || dispute["currency"].as_str() != Some(order.offer.currency.as_str())
            || dispute["livemode"].as_bool() != Some(order.livemode)
        {
            return Err("billingRevenueAdverseEvidenceMismatch".into());
        }
        (
            dispute["amount"]
                .as_u64()
                .ok_or("billingRevenueAdverseEvidenceMismatch")?,
            field(dispute, "status")?,
        )
    } else {
        (0, "none")
    };
    // Different observations of one redelivered event remain immutable. A later
    // provider cumulative total can never be lost behind webhook deduplication.
    let observation_id = format!("{event_id}:refund:{refunded}:dispute:{amount}:{status}");
    Ok(VerifiedPaymentAdverseEvent {
        provider: "stripe".into(),
        source_event_id: observation_id,
        order_id: order.id.clone(),
        payment_intent_id: field(charge, "payment_intent")?.into(),
        livemode: order.livemode,
        currency: order.offer.currency.clone(),
        gross_amount_minor: order.offer.amount_minor,
        cumulative_refunded_minor: refunded,
        disputed_minor: amount,
        dispute_status: status.into(),
        occurred_at_ms: occurred,
        observed_at_ms: now,
        evidence_ref: format!("stripe:charge:{}:event:{event_id}", field(charge, "id")?),
    })
}

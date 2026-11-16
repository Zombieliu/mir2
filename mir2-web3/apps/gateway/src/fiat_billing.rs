//! Explicit player-selected fiat providers. A retry never changes providers.
use crate::{stripe_billing::StripeBilling, waffo_billing::WaffoBilling};
use mir2_simulation::{
    billing::{FiatPaymentProvider, RechargeOffer, RechargeOrder},
    SimulationConfig,
};
use serde_json::{json, Value};

#[derive(Clone)]
pub(crate) enum FiatBilling {
    Stripe(StripeBilling),
    Waffo(WaffoBilling),
}
impl FiatBilling {
    pub(crate) fn from_env(id: FiatPaymentProvider) -> Result<Option<Self>, String> {
        match id {
            FiatPaymentProvider::Stripe => Ok(StripeBilling::from_env()?.map(Self::Stripe)),
            FiatPaymentProvider::Waffo => Ok(WaffoBilling::from_env()?.map(Self::Waffo)),
        }
    }
    pub(crate) fn id(&self) -> FiatPaymentProvider {
        match self {
            Self::Stripe(_) => FiatPaymentProvider::Stripe,
            Self::Waffo(_) => FiatPaymentProvider::Waffo,
        }
    }
    pub(crate) fn offers(&self) -> &[RechargeOffer] {
        match self {
            Self::Stripe(p) => &p.offers,
            Self::Waffo(p) => &p.offers,
        }
    }
    pub(crate) fn livemode(&self) -> bool {
        match self {
            Self::Stripe(p) => p.livemode,
            Self::Waffo(p) => p.livemode,
        }
    }
    pub(crate) fn validate_store(&self, config: &SimulationConfig) -> Result<(), String> {
        match self {
            Self::Stripe(p) => p.validate_store(config),
            Self::Waffo(p) => p.validate_store(config),
        }
    }
    pub(crate) async fn checkout(
        &self,
        config: &SimulationConfig,
        account: &str,
        order: &RechargeOrder,
    ) -> Result<Option<String>, String> {
        if order.provider != self.id() {
            return Err("billingPaymentBindingMismatch".into());
        }
        match self {
            Self::Stripe(p) => p.checkout(config, account, order).await.map(Some),
            Self::Waffo(p) => p.checkout(config, account, order).await,
        }
    }
}

pub(crate) fn public_payment_providers(
    config: &SimulationConfig,
    stripe: Option<&StripeBilling>,
) -> Result<Vec<Value>, String> {
    let mut providers = Vec::new();
    if let Some(p) = stripe {
        p.validate_store(config)?;
        providers.push(json!({"id":"stripe","label":"Stripe","livemode":p.livemode,"offers":p.public_offers()}));
    }
    if let Some(p) = WaffoBilling::from_env()? {
        p.validate_store(config)?;
        providers.push(
            json!({"id":"waffo","label":"WAFFO","livemode":p.livemode,"offers":p.public_offers()}),
        );
    }
    Ok(providers)
}

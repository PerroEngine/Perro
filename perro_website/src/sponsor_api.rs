use axum::{
    extract::{rejection::JsonRejection, Extension},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

const DEFAULT_PORTAL_URL: &str = "https://billing.stripe.com/p/login/00wcN62zc0AEcfG9ag4ZG00";

#[derive(Clone)]
pub struct SponsorState {
    client: reqwest::Client,
    api_base: String,
    secret: Option<String>,
    site_url: String,
    portal_url: String,
    prices: BTreeMap<u16, String>,
}

impl SponsorState {
    pub fn from_env() -> Self {
        let mut prices = BTreeMap::new();
        for (id, key) in price_env_keys() {
            if let Ok(value) = std::env::var(key) {
                let value = value.trim();
                if !value.is_empty() {
                    prices.insert(id, value.to_string());
                }
            }
        }
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            api_base: "https://api.stripe.com".to_string(),
            secret: std::env::var("STRIPE_SECRET_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            site_url: std::env::var("PERRO_SITE_URL")
                .unwrap_or_else(|_| "https://perroengine.com".to_string())
                .trim()
                .trim_end_matches('/')
                .to_string(),
            portal_url: std::env::var("STRIPE_PORTAL_URL")
                .unwrap_or_else(|_| DEFAULT_PORTAL_URL.to_string())
                .trim()
                .to_string(),
            prices,
        }
    }

    #[cfg(test)]
    fn test(api_base: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_base,
            secret: Some("sk_test_perro".to_string()),
            site_url: "http://localhost:3000".to_string(),
            portal_url: DEFAULT_PORTAL_URL.to_string(),
            prices: BTreeMap::from([(1, "price_monthly_bronze".to_string())]),
        }
    }

    #[cfg(test)]
    fn unconfigured() -> Self {
        let mut state = Self::test("http://127.0.0.1:9".to_string());
        state.secret = None;
        state
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SponsorRequest {
    pub id: u16,
    pub amount: Option<u64>,
}

#[derive(Serialize)]
pub struct SponsorResponse {
    url: String,
}

#[derive(Serialize)]
struct ApiError {
    code: &'static str,
    message: &'static str,
}

#[derive(Deserialize)]
struct StripeSession {
    url: Option<String>,
}

pub async fn create_checkout(
    Extension(state): Extension<SponsorState>,
    request: Result<Json<SponsorRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(request)) = request else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Use JSON with a numeric tier id and optional amount.",
        )
        .into_response();
    };
    match create_checkout_for(&state, request).await {
        Ok(url) => (StatusCode::OK, Json(SponsorResponse { url })).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn portal_redirect(Extension(state): Extension<SponsorState>) -> Response {
    if state.portal_url.starts_with("https://billing.stripe.com/") {
        Redirect::temporary(&state.portal_url).into_response()
    } else {
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "stripe_unconfigured",
            "Sponsor billing portal is not configured.",
        )
        .into_response()
    }
}

async fn create_checkout_for(
    state: &SponsorState,
    request: SponsorRequest,
) -> Result<String, SponsorError> {
    let mut form = vec![
        (
            "success_url".to_string(),
            format!("{}/sponsor?checkout=success", state.site_url),
        ),
        (
            "cancel_url".to_string(),
            format!("{}/sponsor?checkout=cancel", state.site_url),
        ),
        ("submit_type".to_string(), "donate".to_string()),
        ("metadata[tier_id]".to_string(), request.id.to_string()),
        ("line_items[0][quantity]".to_string(), "1".to_string()),
    ];

    if request.id == 0 {
        let amount = request
            .amount
            .filter(|amount| (1..=99_999).contains(amount))
            .ok_or_else(|| {
                SponsorError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_amount",
                    "One-time support must be between $1 and $99,999.",
                )
            })?;
        let cents = amount.checked_mul(100).ok_or_else(|| {
            SponsorError::new(
                StatusCode::BAD_REQUEST,
                "invalid_amount",
                "One-time support amount is too large.",
            )
        })?;
        form.extend([
            ("mode".to_string(), "payment".to_string()),
            (
                "line_items[0][price_data][currency]".to_string(),
                "usd".to_string(),
            ),
            (
                "line_items[0][price_data][product_data][name]".to_string(),
                "Perro Engine one-time support".to_string(),
            ),
            (
                "line_items[0][price_data][unit_amount]".to_string(),
                cents.to_string(),
            ),
        ]);
    } else {
        if !is_fixed_tier(request.id) {
            return Err(SponsorError::new(
                StatusCode::BAD_REQUEST,
                "invalid_tier",
                "Unknown sponsor tier.",
            ));
        }
        let price = state.prices.get(&request.id).ok_or_else(|| {
            SponsorError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "stripe_unconfigured",
                "This sponsor tier is not configured.",
            )
        })?;
        form.extend([
            ("mode".to_string(), "subscription".to_string()),
            ("line_items[0][price]".to_string(), price.clone()),
        ]);
    }

    let secret = state.secret.as_deref().ok_or_else(|| {
        SponsorError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "stripe_unconfigured",
            "Sponsor checkout is not configured.",
        )
    })?;
    let response = state
        .client
        .post(format!("{}/v1/checkout/sessions", state.api_base))
        .bearer_auth(secret)
        .form(&form)
        .send()
        .await
        .map_err(|_| {
            SponsorError::new(
                StatusCode::BAD_GATEWAY,
                "stripe_error",
                "Stripe checkout did not respond.",
            )
        })?;

    if !response.status().is_success() {
        return Err(SponsorError::new(
            StatusCode::BAD_GATEWAY,
            "stripe_error",
            "Stripe could not create checkout.",
        ));
    }
    let session = response.json::<StripeSession>().await.map_err(|_| {
        SponsorError::new(
            StatusCode::BAD_GATEWAY,
            "stripe_error",
            "Stripe returned an invalid checkout response.",
        )
    })?;
    session
        .url
        .filter(|url| url.starts_with("https://checkout.stripe.com/"))
        .ok_or_else(|| {
            SponsorError::new(
                StatusCode::BAD_GATEWAY,
                "stripe_error",
                "Stripe returned an invalid checkout URL.",
            )
        })
}

fn is_fixed_tier(id: u16) -> bool {
    matches!(id, 1..=7 | 101..=107)
}

fn price_env_keys() -> [(u16, &'static str); 14] {
    [
        (1, "STRIPE_PRICE_MONTHLY_BRONZE"),
        (2, "STRIPE_PRICE_MONTHLY_SILVER"),
        (3, "STRIPE_PRICE_MONTHLY_GOLD"),
        (4, "STRIPE_PRICE_MONTHLY_PLATINUM"),
        (5, "STRIPE_PRICE_MONTHLY_TITANIUM"),
        (6, "STRIPE_PRICE_MONTHLY_DIAMOND"),
        (7, "STRIPE_PRICE_MONTHLY_EMERALD"),
        (101, "STRIPE_PRICE_CORPORATE_BRONZE"),
        (102, "STRIPE_PRICE_CORPORATE_SILVER"),
        (103, "STRIPE_PRICE_CORPORATE_GOLD"),
        (104, "STRIPE_PRICE_CORPORATE_PLATINUM"),
        (105, "STRIPE_PRICE_CORPORATE_TITANIUM"),
        (106, "STRIPE_PRICE_CORPORATE_DIAMOND"),
        (107, "STRIPE_PRICE_CORPORATE_EMERALD"),
    ]
}

#[derive(Debug)]
struct SponsorError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl SponsorError {
    const fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }

    fn into_response(self) -> Response {
        api_error(self.status, self.code, self.message).into_response()
    }
}

fn api_error(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { code, message }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Router};

    #[test]
    fn accepts_only_known_fixed_tiers() {
        for id in 1..=7 {
            assert!(is_fixed_tier(id));
        }
        for id in 101..=107 {
            assert!(is_fixed_tier(id));
        }
        for id in [8, 99, 100, 108, u16::MAX] {
            assert!(!is_fixed_tier(id));
        }
    }

    #[tokio::test]
    async fn creates_fixed_checkout_from_server_price() {
        let app = Router::new().route(
            "/v1/checkout/sessions",
            post(|| async {
                Json(serde_json::json!({
                    "url": "https://checkout.stripe.com/c/pay/cs_test_perro"
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Stripe");
        let address = listener.local_addr().expect("mock Stripe address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock Stripe");
        });
        let state = SponsorState::test(format!("http://{address}"));
        let url = create_checkout_for(
            &state,
            SponsorRequest {
                id: 1,
                amount: None,
            },
        )
        .await
        .expect("checkout URL");
        assert!(url.starts_with("https://checkout.stripe.com/"));
    }

    #[tokio::test]
    async fn rejects_bad_custom_amounts() {
        let state = SponsorState::test("http://127.0.0.1:9".to_string());
        for amount in [None, Some(0), Some(100_000)] {
            let error = create_checkout_for(&state, SponsorRequest { id: 0, amount })
                .await
                .expect_err("invalid amount");
            assert_eq!(error.status, StatusCode::BAD_REQUEST);
            assert_eq!(error.code, "invalid_amount");
        }
    }

    #[test]
    fn rejects_bad_request_shapes() {
        for body in [
            "{}",
            r#"{"id":"1","amount":null}"#,
            r#"{"id":-1,"amount":null}"#,
            r#"{"id":1,"amount":"five"}"#,
        ] {
            assert!(serde_json::from_str::<SponsorRequest>(body).is_err());
        }
    }

    #[tokio::test]
    async fn reports_missing_server_config() {
        let error = create_checkout_for(
            &SponsorState::unconfigured(),
            SponsorRequest {
                id: 1,
                amount: None,
            },
        )
        .await
        .expect_err("missing config");

        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.code, "stripe_unconfigured");
    }

    #[tokio::test]
    async fn converts_one_time_dollars_to_cents() {
        let app = Router::new().route(
            "/v1/checkout/sessions",
            post(|body: String| async move {
                assert!(body.contains("unit_amount%5D=2500"));
                Json(serde_json::json!({
                    "url": "https://checkout.stripe.com/c/pay/cs_test_once"
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Stripe");
        let address = listener.local_addr().expect("mock Stripe address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock Stripe");
        });
        let url = create_checkout_for(
            &SponsorState::test(format!("http://{address}")),
            SponsorRequest {
                id: 0,
                amount: Some(25),
            },
        )
        .await
        .expect("checkout URL");

        assert!(url.ends_with("cs_test_once"));
    }

    #[tokio::test]
    async fn maps_stripe_faults_to_safe_error() {
        let app = Router::new().route(
            "/v1/checkout/sessions",
            post(|| async { StatusCode::BAD_REQUEST }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Stripe");
        let address = listener.local_addr().expect("mock Stripe address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock Stripe");
        });
        let error = create_checkout_for(
            &SponsorState::test(format!("http://{address}")),
            SponsorRequest {
                id: 1,
                amount: None,
            },
        )
        .await
        .expect_err("Stripe error");

        assert_eq!(error.status, StatusCode::BAD_GATEWAY);
        assert_eq!(error.code, "stripe_error");
    }
}

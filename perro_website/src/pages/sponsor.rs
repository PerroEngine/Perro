use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::layout::PageFrame;
use crate::shared::{Seo, SeoInfo};

struct SponsorTier {
    id: u16,
    name: &'static str,
    price: &'static str,
    perks: &'static [&'static str],
    image: &'static str,
    tone: &'static str,
}

const MONTHLY_TIERS: &[SponsorTier] = &[
    SponsorTier {
        id: 1,
        name: "Bronze Supporter",
        price: "$5 / month",
        perks: &["Fund steady engine work"],
        image: "/tiers/bronze.png",
        tone: "bronze",
    },
    SponsorTier {
        id: 2,
        name: "Silver Dog",
        price: "$10 / month",
        perks: &["Fund features and tooling"],
        image: "/tiers/silver.png",
        tone: "silver",
    },
    SponsorTier {
        id: 3,
        name: "Gold Hound",
        price: "$25 / month",
        perks: &["Name in credits"],
        image: "/tiers/gold.png",
        tone: "gold",
    },
    SponsorTier {
        id: 4,
        name: "Platinum Poodle",
        price: "$45 / month",
        perks: &["Name in credits"],
        image: "/tiers/plat.png",
        tone: "platinum",
    },
    SponsorTier {
        id: 5,
        name: "Titanium Shepherd",
        price: "$75 / month",
        perks: &["Name in credits", "Link in credits"],
        image: "/tiers/titan.png",
        tone: "titanium",
    },
    SponsorTier {
        id: 6,
        name: "Diamond Direwolf",
        price: "$125 / month",
        perks: &["Name in credits", "Link in credits"],
        image: "/tiers/diamond.png",
        tone: "diamond",
    },
    SponsorTier {
        id: 7,
        name: "Emerald Alpha",
        price: "$250 / month",
        perks: &["Logo in credits", "Link in credits", "Special thanks"],
        image: "/tiers/emerald.png",
        tone: "emerald",
    },
];

const CORPORATE_TIERS: &[SponsorTier] = &[
    SponsorTier {
        id: 101,
        name: "Corporate Bronze",
        price: "$500 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/bronze.png",
        tone: "bronze",
    },
    SponsorTier {
        id: 102,
        name: "Corporate Silver",
        price: "$1,000 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/silver.png",
        tone: "silver",
    },
    SponsorTier {
        id: 103,
        name: "Corporate Gold",
        price: "$2,500 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/gold.png",
        tone: "gold",
    },
    SponsorTier {
        id: 104,
        name: "Corporate Platinum",
        price: "$5,000 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/plat.png",
        tone: "platinum",
    },
    SponsorTier {
        id: 105,
        name: "Corporate Titanium",
        price: "$7,500 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/titan.png",
        tone: "titanium",
    },
    SponsorTier {
        id: 106,
        name: "Corporate Diamond",
        price: "$10,000 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/diamond.png",
        tone: "diamond",
    },
    SponsorTier {
        id: 107,
        name: "Corporate Emerald",
        price: "$15,000 / month",
        perks: &["Logo in credits", "Link in credits", "Sponsor highlight"],
        image: "/tiers/emerald.png",
        tone: "emerald",
    },
];

#[component]
pub fn SponsorPage() -> impl IntoView {
    let query = use_query_map();
    let checkout = move || query.with(|map| map.get("checkout"));

    view! {
        <Seo info=SeoInfo::new(
            "Sponsor",
            "Support Perro open-source Rust game engine work across runtime optimization, platform support, docs, examples, demos, tooling, and community growth.",
            "sponsor Perro, open source game engine funding, Rust game engine sponsor, game engine docs, Perro demos",
            "/sponsor",
        ) />
        <PageFrame eyebrow="Sponsor" title="Help Perro keep moving.">
            <section class="sponsor-hero">
                <img class="sponsor-dog" src="/tiers/perro-trans.png" alt="" />
                <div>
                    <p class="lead">
                        "Sponsor a fast, understandable, open-source Rust game engine. Support funds runtime work, platform coverage, docs, demos, and the tools around them."
                    </p>
                    <a class="sponsor-manage" href="/api/sponsor/portal">"Manage an existing donation ↗"</a>
                </div>
            </section>

            {move || match checkout().as_deref() {
                Some("success") => view! {
                    <div class="checkout-banner success" role="status">
                        <strong>"Thank you."</strong>
                        <span>"Stripe accepted the checkout. Your support keeps Perro moving."</span>
                    </div>
                }.into_any(),
                Some("cancel") => view! {
                    <div class="checkout-banner cancel" role="status">
                        <strong>"Checkout canceled."</strong>
                        <span>"Nothing was charged. Pick any tier when you are ready."</span>
                    </div>
                }.into_any(),
                _ => view! { <span></span> }.into_any(),
            }}

            <div class="sponsor-tabs">
                <input class="sponsor-tab-input" id="sponsor-monthly" name="sponsor-mode" type="radio" checked />
                <input class="sponsor-tab-input" id="sponsor-one-time" name="sponsor-mode" type="radio" />
                <input class="sponsor-tab-input" id="sponsor-corporate" name="sponsor-mode" type="radio" />

                <div class="sponsor-switch" role="tablist" aria-label="Sponsor type">
                    <span class="sponsor-switch-thumb"></span>
                    <label class="monthly-tab" for="sponsor-monthly" role="tab">"Monthly"</label>
                    <label class="one-time-tab" for="sponsor-one-time" role="tab">"One time"</label>
                    <label class="corporate-tab" for="sponsor-corporate" role="tab">"Corporate"</label>
                </div>

                <SponsorTierPanel class_name="monthly-panel" tiers=MONTHLY_TIERS />
                <OneTimePanel />
                <SponsorTierPanel class_name="corporate-panel" tiers=CORPORATE_TIERS />
            </div>

            <p id="sponsor-error" class="sponsor-error" role="alert" aria-live="polite"></p>

            <section class="band sponsor-note">
                <div>
                    <p class="eyebrow">"No payment needed"</p>
                    <h2>"Other ways to help"</h2>
                </div>
                <div class="support-grid">
                    <SupportCard title="Contribute" body="Open issues, docs, demos, engine systems, and tests." href="https://github.com/PerroEngine/Perro" />
                    <SupportCard title="Share" body="Build a demo, write notes, and show what Perro can make." href="/community" />
                    <SupportCard title="Report" body="File focused bugs with repro steps and platform details." href="https://github.com/PerroEngine/Perro/issues" />
                </div>
            </section>

            <p class="sponsor-legal">
                "Payments are securely processed through Stripe and billed by DeFranco Studios Inc., the entity authorized to accept support for Perro Engine."
            </p>
            <SponsorScript />
        </PageFrame>
    }
}

#[component]
fn SponsorTierPanel(class_name: &'static str, tiers: &'static [SponsorTier]) -> impl IntoView {
    view! {
        <section class=format!("sponsor-section sponsor-panel {class_name}")>
            <div class="sponsor-grid">
                {tiers.iter().map(|tier| view! { <SponsorTierCard tier=tier /> }).collect_view()}
            </div>
        </section>
    }
}

#[component]
fn SponsorTierCard(tier: &'static SponsorTier) -> impl IntoView {
    view! {
        <article class=format!("sponsor-card {}", tier.tone)>
            <div class="tier-top">
                <img src=tier.image alt=format!("{} tier badge", tier.name) loading="lazy" />
                <h3>{tier.name}</h3>
                <strong>{tier.price}</strong>
            </div>
            <ul>
                {tier.perks.iter().map(|perk| view! { <li>{*perk}</li> }).collect_view()}
            </ul>
            <button class="btn primary sponsor-submit" type="button" data-sponsor-id=tier.id>
                "Support"
            </button>
        </article>
    }
}

#[component]
fn OneTimePanel() -> impl IntoView {
    view! {
        <section class="sponsor-section sponsor-panel one-time-panel">
            <article class="sponsor-card one-time-card">
                <img src="/tiers/perro-trans.png" alt="" />
                <p class="eyebrow">"One-time support"</p>
                <h3>"Choose your amount"</h3>
                <p>"Make one secure Stripe payment. No subscription."</p>
                <div class="amount-presets" aria-label="Suggested amounts">
                    {[25_u64, 50, 75, 100, 250, 500].into_iter().map(|amount| view! {
                        <button type="button" class="amount-preset" data-amount=amount>
                            {format!("${amount}")}
                        </button>
                    }).collect_view()}
                </div>
                <label for="one-time-amount">"Custom amount in USD"</label>
                <div class="custom-amount">
                    <span>"$"</span>
                    <input id="one-time-amount" type="number" min="1" max="99999" step="1" inputmode="numeric" placeholder="50" />
                </div>
                <button class="btn primary sponsor-submit" type="button" data-sponsor-id="0">
                    "Continue to Stripe"
                </button>
            </article>
        </section>
    }
}

#[component]
fn SponsorScript() -> impl IntoView {
    view! {
        <script>
            {r#"
const sponsorError = document.getElementById("sponsor-error");
const amountInput = document.getElementById("one-time-amount");
for (const preset of document.querySelectorAll(".amount-preset")) {
  preset.addEventListener("click", () => {
    amountInput.value = preset.dataset.amount;
    amountInput.focus();
  });
}
for (const button of document.querySelectorAll(".sponsor-submit")) {
  button.addEventListener("click", async () => {
    if (button.dataset.busy === "1") return;
    const id = Number(button.dataset.sponsorId);
    const amount = id === 0 ? Number(amountInput.value) : null;
    sponsorError.textContent = "";
    button.dataset.busy = "1";
    button.disabled = true;
    const oldText = button.textContent;
    button.textContent = "Opening Stripe…";
    try {
      const response = await fetch("/api/sponsor", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id, amount })
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.message || "Checkout could not start.");
      if (!data.url || !data.url.startsWith("https://checkout.stripe.com/")) {
        throw new Error("Stripe returned an invalid checkout link.");
      }
      location.assign(data.url);
    } catch (error) {
      sponsorError.textContent = error.message || "Checkout could not start.";
      button.dataset.busy = "0";
      button.disabled = false;
      button.textContent = oldText;
    }
  });
}
"#}
        </script>
    }
}

#[component]
fn SupportCard(title: &'static str, body: &'static str, href: &'static str) -> impl IntoView {
    view! {
        <a class="feature-card" href=href>
            <h3>{title}</h3>
            <p>{body}</p>
        </a>
    }
}

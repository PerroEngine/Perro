use leptos::prelude::*;

use crate::layout::PageFrame;
use crate::shared::{Seo, SeoInfo};

struct SponsorTier {
    name: &'static str,
    price: &'static str,
    perks: &'static [&'static str],
    image: &'static str,
    tone: &'static str,
    href: &'static str,
}

const MONTHLY_TIERS: &[SponsorTier] = &[
    SponsorTier {
        name: "Bronze Supporter",
        price: "$5 / month",
        perks: &["Support engine work"],
        image: "/tiers/bronze.png",
        tone: "bronze",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Silver Dog",
        price: "$10 / month",
        perks: &["Support features + tooling"],
        image: "/tiers/silver.png",
        tone: "silver",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Gold Hound",
        price: "$25 / month",
        perks: &["Name in credits"],
        image: "/tiers/gold.png",
        tone: "gold",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Platinum Poodle",
        price: "$45 / month",
        perks: &["Name in credits"],
        image: "/tiers/plat.png",
        tone: "platinum",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Titanium Shepherd",
        price: "$75 / month",
        perks: &["Name in credits", "Link in credits"],
        image: "/tiers/titan.png",
        tone: "titanium",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Diamond Direwolf",
        price: "$125 / month",
        perks: &["Name in credits", "Link in credits"],
        image: "/tiers/diamond.png",
        tone: "diamond",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Emerald Alpha",
        price: "$250 / month",
        perks: &["Logo in credits", "Link in credits", "Special thanks"],
        image: "/tiers/emerald.png",
        tone: "emerald",
        href: "https://ko-fi.com/perroengine",
    },
];

const ONE_TIME_TIERS: &[SponsorTier] = &[
    SponsorTier {
        name: "Bronze Gift",
        price: "$5 once",
        perks: &["Support engine work"],
        image: "/tiers/bronze.png",
        tone: "bronze",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Silver Gift",
        price: "$15 once",
        perks: &["Support features + tooling"],
        image: "/tiers/silver.png",
        tone: "silver",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Gold Gift",
        price: "$50 once",
        perks: &["Name in credits"],
        image: "/tiers/gold.png",
        tone: "gold",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Platinum Gift",
        price: "$100 once",
        perks: &["Name in credits", "Link in credits"],
        image: "/tiers/plat.png",
        tone: "platinum",
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Diamond Gift",
        price: "$250 once",
        perks: &["Logo in credits", "Link in credits", "Special thanks"],
        image: "/tiers/diamond.png",
        tone: "diamond",
        href: "https://ko-fi.com/perroengine",
    },
];

const CORPORATE_TIERS: &[SponsorTier] = &[
    SponsorTier {
        name: "Corporate Bronze",
        price: "$500 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/bronze.png",
        tone: "bronze",
        href: "mailto:support@perroengine.com?subject=Corporate%20Sponsorship",
    },
    SponsorTier {
        name: "Corporate Silver",
        price: "$1,000 / month",
        perks: &["Logo in credits", "Link in credits"],
        image: "/tiers/silver.png",
        tone: "silver",
        href: "mailto:support@perroengine.com?subject=Corporate%20Sponsorship",
    },
    SponsorTier {
        name: "Corporate Gold",
        price: "$2,000 / month",
        perks: &["Logo in credits", "Link in credits", "Sponsor highlight"],
        image: "/tiers/gold.png",
        tone: "gold",
        href: "mailto:support@perroengine.com?subject=Corporate%20Sponsorship",
    },
];

#[component]
pub fn SponsorPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Sponsor",
            "Support Perro open-source Rust game engine work across runtime optimization, platform support, docs, examples, demos, tooling, and community growth.",
            "sponsor Perro, open source game engine funding, Rust game engine sponsor, game engine docs, Perro demos",
            "/sponsor",
        ) />
        <PageFrame eyebrow="Sponsor" title="Support Perro">
            <section class="sponsor-hero">
                <p class="lead">
                    "Donate and support Perro Engine's mission to create an open-source, high-performance, simple game engine. Your generosity funds new features, optimization, platform support, and community growth."
                </p>
                <a class="sponsor-manage" href="https://ko-fi.com/perroengine" target="_blank" rel="noreferrer">"Manage Donation"</a>
            </section>

            <div class="sponsor-tabs">
                <input class="sponsor-tab-input" id="sponsor-monthly" name="sponsor-mode" type="radio" checked />
                <input class="sponsor-tab-input" id="sponsor-one-time" name="sponsor-mode" type="radio" />
                <input class="sponsor-tab-input" id="sponsor-corporate" name="sponsor-mode" type="radio" />

                <div class="sponsor-switch">
                    <span class="sponsor-switch-thumb"></span>
                    <label class="monthly-tab" for="sponsor-monthly">"Monthly"</label>
                    <label class="one-time-tab" for="sponsor-one-time">"One Time"</label>
                    <label class="corporate-tab" for="sponsor-corporate">"Corporate"</label>
                </div>

                <SponsorTierPanel class_name="monthly-panel" tiers=MONTHLY_TIERS />
                <SponsorTierPanel class_name="one-time-panel" tiers=ONE_TIME_TIERS />
                <SponsorTierPanel class_name="corporate-panel" tiers=CORPORATE_TIERS />
            </div>

            <section class="band sponsor-note">
                <h2>"Other ways to help"</h2>
                <div class="support-grid">
                    <SupportCard title="Contribute" body="Open issues, docs, demos, engine systems, and tests." href="https://github.com/PerroEngine/Perro" />
                    <SupportCard title="Share" body="Build demos, write notes, and show Perro projects." href="/community" />
                    <SupportCard title="Report" body="File clear bugs with repro steps and target platform info." href="https://github.com/PerroEngine/Perro/issues" />
                </div>
            </section>
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
            <a class="btn primary" href=tier.href target="_blank" rel="noreferrer">"Support"</a>
        </article>
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

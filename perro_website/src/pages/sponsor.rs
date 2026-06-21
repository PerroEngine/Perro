use leptos::prelude::*;

use crate::layout::PageFrame;
use crate::shared::{Seo, SeoInfo};

struct SponsorTier {
    name: &'static str,
    price: &'static str,
    perks: &'static [&'static str],
    href: &'static str,
}

const MONTHLY_TIERS: &[SponsorTier] = &[
    SponsorTier {
        name: "Bronze Supporter",
        price: "$5 / month",
        perks: &["Support engine work"],
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Silver Dog",
        price: "$10 / month",
        perks: &["Support features + tooling"],
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Gold Hound",
        price: "$25 / month",
        perks: &["Name in credits"],
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Platinum Poodle",
        price: "$45 / month",
        perks: &["Name in credits"],
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Titanium Shepherd",
        price: "$75 / month",
        perks: &["Name in credits", "Link in credits"],
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Diamond Direwolf",
        price: "$125 / month",
        perks: &["Name in credits", "Link in credits"],
        href: "https://ko-fi.com/perroengine",
    },
    SponsorTier {
        name: "Emerald Alpha",
        price: "$250 / month",
        perks: &["Logo in credits", "Link in credits", "Special thanks"],
        href: "https://ko-fi.com/perroengine",
    },
];

const CORPORATE_TIERS: &[SponsorTier] = &[
    SponsorTier {
        name: "Corporate Bronze",
        price: "$500 / month",
        perks: &["Logo in credits", "Link in credits"],
        href: "mailto:support@perroengine.com?subject=Corporate%20Sponsorship",
    },
    SponsorTier {
        name: "Corporate Silver",
        price: "$1,000 / month",
        perks: &["Logo in credits", "Link in credits"],
        href: "mailto:support@perroengine.com?subject=Corporate%20Sponsorship",
    },
    SponsorTier {
        name: "Corporate Gold",
        price: "$2,000 / month",
        perks: &["Logo in credits", "Link in credits", "Sponsor highlight"],
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
                    "Donate to support Perro's mission: an open-source, high-performance, simple game engine. Support funds engine features, optimization, platform work, docs, and community growth."
                </p>
                <div class="sponsor-actions">
                    <a class="btn primary" href="https://ko-fi.com/perroengine" target="_blank" rel="noreferrer">"Donate on Ko-fi"</a>
                    <a class="btn ghost" href="https://github.com/PerroEngine/Perro" target="_blank" rel="noreferrer">"Sponsor via GitHub"</a>
                </div>
            </section>

            <SponsorTierSection title="Monthly" tiers=MONTHLY_TIERS />
            <SponsorTierSection title="Corporate" tiers=CORPORATE_TIERS />

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
fn SponsorTierSection(title: &'static str, tiers: &'static [SponsorTier]) -> impl IntoView {
    view! {
        <section class="band sponsor-section">
            <div class="section-head row">
                <div>
                    <p class="eyebrow">"Donation tiers"</p>
                    <h2>{title}</h2>
                </div>
            </div>
            <div class="sponsor-grid">
                {tiers.iter().map(|tier| view! { <SponsorTierCard tier=tier /> }).collect_view()}
            </div>
        </section>
    }
}

#[component]
fn SponsorTierCard(tier: &'static SponsorTier) -> impl IntoView {
    view! {
        <article class="sponsor-card">
            <div>
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

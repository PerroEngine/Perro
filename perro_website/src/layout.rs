use leptos::prelude::*;

#[component]
pub fn SiteShell(children: Children) -> impl IntoView {
    view! {
        <div class="site">
            <a class="skip-link" href="#main-content">"Skip to content"</a>
            <header class="topbar">
                <a class="brand" href="/">
                    <img src="/perro.svg" alt="" />
                    <span>
                        <strong>"Perro"</strong>
                        <small>"Rust game engine"</small>
                    </span>
                </a>
                <nav class="nav" aria-label="Main navigation">
                    <a data-nav href="/features">"Product"</a>
                    <a data-nav href="/book">"Learn"</a>
                    <a data-nav href="/docs">"Docs"</a>
                    <a data-nav href="/nodes">"Nodes"</a>
                    <a data-nav href="/examples">"Examples"</a>
                    <a data-nav href="/community">"Community"</a>
                </nav>
                <div class="nav-actions">
                    <a class="github-link" href="https://github.com/PerroEngine/Perro" target="_blank" rel="noreferrer">"GitHub ↗"</a>
                    <a class="pill" href="/sponsor">"Sponsor Perro"</a>
                </div>
                <details class="mobile-menu">
                    <summary aria-label="Open navigation">"Menu"</summary>
                    <nav aria-label="Mobile navigation">
                        <a href="/features">"Product"</a>
                        <a href="/mission">"Mission"</a>
                        <a href="/book">"Learn"</a>
                        <a href="/docs">"Docs"</a>
                        <a href="/nodes">"Nodes"</a>
                        <a href="/examples">"Examples"</a>
                        <a href="/community">"Community"</a>
                        <a href="/sponsor">"Sponsor"</a>
                    </nav>
                </details>
            </header>
            <div id="main-content" tabindex="-1">{children()}</div>
            <footer class="site-footer">
                <div class="footer-brand">
                    <img src="/perro.svg" alt="" />
                    <div>
                        <strong>"Perro Engine"</strong>
                        <p>"Small authoring loop. Fast Rust runtime. Open source."</p>
                    </div>
                </div>
                <div class="footer-links">
                    <div>
                        <strong>"Build"</strong>
                        <a href="/learn/getting-started">"Get started"</a>
                        <a href="/book">"Book"</a>
                        <a href="/examples">"Examples"</a>
                    </div>
                    <div>
                        <strong>"Reference"</strong>
                        <a href="/docs">"Docs"</a>
                        <a href="/nodes">"Node registry"</a>
                        <a href="/assets">"Assets"</a>
                    </div>
                    <div>
                        <strong>"Project"</strong>
                        <a href="/mission">"Mission"</a>
                        <a href="/community">"Community"</a>
                        <a href="/news">"Project notes"</a>
                    </div>
                    <div>
                        <strong>"Support"</strong>
                        <a href="/sponsor">"Sponsor"</a>
                        <a href="https://github.com/PerroEngine/Perro">"GitHub ↗"</a>
                        <a href="https://github.com/PerroEngine/Perro/blob/main/LICENSE">"Apache-2.0 ↗"</a>
                    </div>
                </div>
                <p class="footer-note">"Perro support payments are processed by Stripe for DeFranco Studios Inc."</p>
            </footer>
            <script>
                {r#"
for (const link of document.querySelectorAll("[data-nav]")) {
  const href = link.getAttribute("href");
  if (href === location.pathname || (href !== "/" && location.pathname.startsWith(href + "/"))) {
    link.setAttribute("aria-current", "page");
  }
}
document.addEventListener("click", async event => {
  const button = event.target.closest?.(".copy-code");
  if (!button) return;
  const code = button.closest(".code-script")?.querySelector("code")?.innerText || "";
  try {
    await navigator.clipboard.writeText(code);
    button.textContent = "Copied";
    setTimeout(() => button.textContent = "Copy", 1400);
  } catch {
    button.textContent = "Copy failed";
  }
});
document.addEventListener("keydown", event => {
  if (event.key !== "/" || event.metaKey || event.ctrlKey || event.altKey) return;
  const target = event.target;
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
  const search = document.getElementById("docs-search");
  if (search) {
    event.preventDefault();
    search.focus();
  }
});
"#}
            </script>
        </div>
    }
}

#[component]
pub fn PageFrame<E, T>(eyebrow: E, title: T, children: Children) -> impl IntoView
where
    E: Into<String> + Clone + Send + Sync + 'static,
    T: Into<String> + Clone + Send + Sync + 'static,
{
    let eyebrow = eyebrow.into();
    let title = title.into();
    view! {
        <main class="page">
            <section class="page-head">
                <p class="eyebrow">{eyebrow}</p>
                <h1>{title}</h1>
                <span class="page-head-mark" aria-hidden="true"></span>
            </section>
            {children()}
        </main>
    }
}

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <main class="page">
            <section class="page-head">
                <p class="eyebrow">"404"</p>
                <h1>"This trail ends here."</h1>
                <p class="lead">"The page moved, the link broke, or the dog ran off with it."</p>
                <div class="page-actions">
                    <a class="btn primary" href="/">"Back home"</a>
                    <a class="btn ghost" href="/docs">"Search docs"</a>
                </div>
            </section>
        </main>
    }
}

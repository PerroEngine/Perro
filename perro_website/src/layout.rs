use leptos::prelude::*;

#[component]
pub fn SiteShell(children: Children) -> impl IntoView {
    view! {
        <div class="site">
            <header class="topbar">
                <a class="brand" href="/">
                    <span>"Perro"</span>
                </a>
                <nav class="nav">
                    <a href="/features">"Features"</a>
                    <a href="/book">"Book"</a>
                    <a href="/nodes">"Nodes"</a>
                    <a href="/learn/getting-started">"Learn"</a>
                    <a href="/docs">"Docs"</a>
                    <a href="/news">"News"</a>
                    <a href="/community">"Community"</a>
                    <a href="/assets">"Assets"</a>
                    <a href="/examples">"Examples"</a>
                </nav>
                <div class="nav-actions">
                    <a href="https://github.com/PerroEngine/Perro">"GitHub"</a>
                    <a class="pill" href="/sponsor">"Sponsor"</a>
                </div>
            </header>
            {children()}
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
                <h1>"Page not found"</h1>
                <a class="btn primary" href="/">"Home"</a>
            </section>
        </main>
    }
}

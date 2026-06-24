use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_meta::{provide_meta_context, MetaTags};
use leptos_router::{components::*, path};

pub mod docs;
mod highlight;
mod layout;
mod pages;
mod shared;

use layout::{NotFound, SiteShell};
use pages::{
    AssetsPage, BookChapterPage, BookPage, CommunityPage, Demo2dPage, Demo3dPage, DocPageView,
    DocsIndexPage, ExamplesPage, FeaturesPage, GetStartedPage, HomePage, NewsPage, NodesPage,
    SponsorPage,
};

#[component]
#[cfg(feature = "ssr")]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <MetaTags />
                <meta name="theme-color" content="#1b1f24" />
                <meta name="application-name" content="Perro Engine" />
                <link rel="icon" href="/perro.svg" type="image/svg+xml" />
                <link rel="stylesheet" href="/style/main.css" />
            </head>
            <body>
                <AppRoutes />
            </body>
        </html>
    }
}

#[component]
#[cfg(not(feature = "ssr"))]
pub fn App() -> impl IntoView {
    view! { <AppRoutes /> }
}

#[component]
fn AppRoutes() -> impl IntoView {
    view! {
        <Router>
            <SiteShell>
                <Routes fallback=NotFound>
                    <Route path=path!("") view=HomePage />
                    <Route path=path!("features") view=FeaturesPage />
                    <Route path=path!("book") view=BookPage />
                    <Route path=path!("book/*slug") view=BookChapterPage />
                    <Route path=path!("nodes") view=NodesPage />
                    <Route path=path!("learn/getting-started") view=GetStartedPage />
                    <Route path=path!("docs") view=DocsIndexPage />
                    <Route path=path!("docs/*slug") view=DocPageView />
                    <Route path=path!("examples") view=ExamplesPage />
                    <Route path=path!("examples/2d") view=Demo2dPage />
                    <Route path=path!("examples/3d") view=Demo3dPage />
                    <Route path=path!("news") view=NewsPage />
                    <Route path=path!("community") view=CommunityPage />
                    <Route path=path!("assets") view=AssetsPage />
                    <Route path=path!("sponsor") view=SponsorPage />
                </Routes>
            </SiteShell>
        </Router>
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(App);
}

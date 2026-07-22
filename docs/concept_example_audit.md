# Concept And Example Audit

## Purpose

Track concept-first coverage for active user docs.

Status meanings:

- `deep`: full mental model, decisions, failure behavior, and feature story
- `compact`: small topic with enough model, choice, and grounded scenario
- `reference`: accurate API data, but concept work remains
- `history`: dated audit, ABI record, or design history; no prose rewrite

## Pass Order

| Group | Target | Required result |
| --- | --- | --- |
| authoring | `docs/scripting/authoring/` | deep guide + verified feature stories |
| scripting core | state, lifecycle, methods, variant, queries, nodes | deep |
| script contexts | runtime, resource, and input modules | deep or compact before API refs |
| engine features | physics, UI, audio, animation, scenes, resources | deep |
| project and platform | project, networking, platform, WASM, tools | deep or compact by complexity |
| book | `perro_book/` | narrative path + links to exact refs |
| demos | demo READMEs | feature maps + why each pattern fits |

## Required Cross-System Stories

| Story | Verified source | Coverage |
| --- | --- | --- |
| fixed node and typed asset injection | ScriptPatterns | state owner, scene vars, init order |
| switch calls door | ScriptPatterns | targeted method + reply |
| player health updates HUD | ScriptPatterns | signal fan-out |
| generic runtime adapter | ScriptPatterns | `get_var!` + `set_var!` |
| delayed action | ScriptPatterns | named timer + finish signal |
| manager and spawned set | Demo2D or Demo3D | query vs fixed refs |
| larger 2D/3D/UI composition | existing demos | real feature ownership |

## Acceptance Checklist

- active page uses one page profile from [Writing Standard](writing_standard.md)
- use cases state situation, choice, reason, and tradeoff
- major example explains ownership and data flow
- failure result appears next to the operation
- typed vs dynamic access stays explicit
- reference anchors and routes stay stable
- verified examples pass check, doctor, and clippy
- historical pages stay history

## 2026-07 Completion

| Group | Result |
| --- | --- |
| authoring | deep guide + seven feature stories + verified source map |
| active scripting | 65/65 non-authoring pages receive concept/choice coverage |
| active project/resource/network/platform/tool docs | concept-first pass complete |
| book | 15/15 pages align to narrative path + exact refs |
| demos | Demo2D, Demo3D, DemoUI READMEs map features to source |
| verified demo | ScriptPatterns check, doctor, clippy clean |
| links | 153 docs/book/demo README files; 0 broken relative links or anchors |

Demo2D + Demo3D check/clippy pass. Doctor reports existing unbound scene-signal
warnings but 0 errors. DemoUI check/doctor/clippy pass.

# Skill: mzizi-component-install

When a Mzizi component is missing, do not write it by hand. Emit an
"install-component" step whose params.components is the bare Mzizi registry
name(s). fundi expands each name to its full registry URL and runs
"npx shadcn@latest add https://api.mzizi.dev/v1/ui/<name>" — do NOT put a
URL in params.components yourself, and do not emit a bare "npx shadcn add".

Examples:
- Missing Button → { kind: "install-component", params: { components: ["button"] } }
- Missing Card + Badge → { kind: "install-component", params: { components: ["card", "badge"] } }

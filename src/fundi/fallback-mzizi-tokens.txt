# Skill: mzizi-tokens

Mzizi token wiring lives in app/globals.css. If the snapshot reports
hasGlobalsCss=false, propose a create-file step at app/globals.css with the
canonical Mzizi token preamble (@layer base { :root { --color-* } }).
If globals.css exists but the caller asked for token alignment, modify it
instead of recreating it.

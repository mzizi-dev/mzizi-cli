# Security policy

Report a vulnerability to **<security@nyuchi.com>**. Do not open a public issue for it.

What `mz` does that matters for security: it writes files into the current project
(component files from api.mzizi.dev; path traversal is rejected and existing files are
kept unless `--overwrite`), reads project files for `explore`, `plan` and `floor`, sends
requests to api.mzizi.dev, crates.io, the MCP endpoint and (with `ANTHROPIC_API_KEY`)
Anthropic, and stores a sign-in token in the OS keychain or a 0600 file. It runs no
downloaded code.

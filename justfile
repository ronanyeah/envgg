generate-cli-doc:
    cargo run -q -p envgg-headless -- markdown-help > docs/CLI.md

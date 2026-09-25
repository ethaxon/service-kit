build *args:
    mise exec -- node scripts/dev-cli.ts build {{args}}

build-docs:
    mise exec -- node scripts/dev-cli.ts build-docs

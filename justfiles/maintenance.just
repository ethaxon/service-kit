link-docs:
    mise exec -- node scripts/dev-cli.ts link-docs

package *args:
    mise exec -- node scripts/dev-cli.ts package {{args}}

dist *args:
    mise exec -- node scripts/dev-cli.ts dist {{args}}

interactive:
    mise exec -- node scripts/dev-cli.ts interactive

sync-metadata:
    mise exec -- node scripts/dev-cli.ts sync-metadata

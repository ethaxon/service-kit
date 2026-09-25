# Bootstrap tools before loading the dependency-free Node setup workflow.
setup:
    mise install
    mise exec -- node scripts/setup.ts

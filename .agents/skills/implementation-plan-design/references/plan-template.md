# Optional Plan Outline

Use only the sections that help the task. Prose and a short list are usually sufficient; tables compare alternatives, diagrams explain complicated flows, and code declarations pin down consequential public contracts. IDs are useful only when repeated cross-references need them.

## Outcome and scope

What user-visible result is required? What material decisions remain? State the current implementation status when useful.

## Design

Describe changed ownership, behavior and interfaces. Include data migration, partial failure, concurrency or external boundaries where they affect correctness. Link source evidence for uncertain APIs and explain consequential choices. Leave routine implementation detail to the code.

## Implementation

List cohesive steps in dependency order, naming the affected app/crate and important files. Split work only where ownership or sequencing warrants it.

## Verification and delivery

Name the smallest checks that establish the changed behavior for the current stage. Reuse existing tests. For iterative development, identify the usable result to hand to the user; do not turn every conceivable scenario into a completion requirement.

Update this section with actual results and material limitations. Do not maintain duplicate work-package and aggregate test ledgers, PR status, or a catalogue of all modified files.

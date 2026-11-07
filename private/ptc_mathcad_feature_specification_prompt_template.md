# Reusable PTC Mathcad Feature Specification Prompt

<feature_configuration>
FEATURE_NAME = "[INSERT FEATURE NAME HERE]"
</feature_configuration>

Act as a Principal Software Architect and PTC Mathcad Systems Expert specializing in high-performance mathematical calculation engines, robust dimensional analysis (unit management systems), and numerical/symbolic solvers. Produce a comprehensive, implementation-ready technical specification for adding the PTC Mathcad feature named by `FEATURE_NAME` to the current repository.

Treat `FEATURE_NAME` as the sole per-feature input. Derive all terminology, command names, region types, subsystem names, and the output filename from it. Do not require any other placeholder replacement. Save the finished specification as detailed Markdown at:

```text
private_material/<feature_name_in_snake_case>_specification.md
```

If a file with that name already exists, inspect it and update it carefully rather than discarding useful verified material. Do not modify application source code; this task produces the specification only.

## Objective

Write the definitive engineering blueprint that a developer can follow to implement `FEATURE_NAME` in the repository. It must be thorough enough to guide data-model changes, mathematical evaluation pipelines, unit-checking algorithms, region rendering, `.mcdx` persistence, validation, testing, and staged delivery without requiring the implementer to invent missing architecture.

The result must satisfy two standards simultaneously:

1. Faithfulness to official PTC Mathcad Prime architecture, calculation engine semantics, unit dimensional analysis, worksheet structure, and developer APIs.
2. Direct compatibility with the repository as it exists now, including its crate/module boundaries, data types, execution framework, undo model, renderer, file formats, coding rules, and currently missing subsystems.

Do not write a generic spreadsheet, scientific calculator, LaTeX renderer, MATLAB, or abstract design-pattern proposal. Do not merely describe an ideal greenfield architecture. Ground every important decision in PTC Mathcad behavior and the live codebase.

## Required working process

### 1. Inspect the live repository before designing

Read the repository's current instructions and architecture completely enough to understand the applicable rules. At minimum inspect:

- `CLAUDE.md`;
- `ARCHITECTURE.md`;
- the relevant current sections of `ui_plan.md`;
- the relevant command/reference documents in `private_material/`;
- workspace and affected crate configuration files (`Cargo.toml` or equivalent);
- the current region types (`MathRegion`, `TextRegion`, `PlotRegion`, etc.), `Document`, `RegionId`, styles, units, validation, and evaluation implementations;
- session state, transaction management, evaluation cache, undo/redo, and command registration implementations;
- native `.mcdx` / worksheet DTOs and the current version history;
- math parsing, AST generation, tokenization, and unit-checking dispatch;
- canvas layout, region positioning, hit-testing, plotting, and existing calculation dependency graphs or caches; and
- `git status`, so unrelated or unfinished user work is not mistaken for established architecture.

Use the code as the authority for what exists today. Do not copy stale version numbers, dependencies, feature gaps, or file paths from older specifications. If documentation and code disagree, call out the discrepancy and design against the code unless the repository explicitly says the documentation is authoritative.

Before proposing a new region variant or shared attribute, locate all exhaustive matches and affected subsystems. Explain the required behavior in each one. Respect the repository rule that dependencies point downward and never place file-format, UI, or evaluation types in core foundational modules.

### 2. Research PTC Mathcad from authoritative sources

Research `FEATURE_NAME` using current, official PTC Mathcad sources wherever available. Prefer, in this order:

1. Official PTC Mathcad Prime Help Center and User Guides.
2. PTC Mathcad Prime API Reference (Automation, Custom Functions, and .NET APIs).
3. Published mathematical engine specifications and unit-dimension consistency standards.
4. Official Mathcad XML schema specifications for `.mcdx` containers.
5. PTC technical support knowledge base articles regarding calculation engine behavior and solve blocks.

Use secondary sources only to locate official material, resolve a gap explicitly left by PTC, or design an interoperability test. Do not present secondary claims as proprietary Mathcad facts.

Research all applicable aspects, including:

- exact PTC Mathcad terminology and class/subclass names;
- region classification (math regions, text regions, plot regions, component regions, image regions);
- scope, variable assignment order (top-to-bottom, left-to-right calculation flow), local vs. global definitions (`:=` vs. `≡`), and solve block structures (`Given`, `Find`, `Minerr`);
- unit dimensions, fundamental base dimensions (mass, length, time, current, temperature, amount of substance, luminous intensity), unit conversion factors, dimensionless checks, and error propagation on dimensional mismatch;
- numeric evaluation vs. symbolic evaluation (`->` operator, MuPAD/internal symbolic engine integration);
- worksheet persistence structure (`.mcdx` zip container, `mathcad.xml`, embedded packages, calculation state cache);
- interactive editing behavior, auto-calculation triggers, error badging, collapsible areas, and formatting options;
- performance limits, recursive calculation guards, and timeout handlers; and
- behavior that changes across Mathcad versions (e.g., Mathcad 15 vs. Mathcad Prime releases).

Use citations as close as possible to the claims they support and finish with a curated official-reference list. Link directly to official PTC documentation pages, not search-results pages.

Clearly label each important statement as one of the following when ambiguity could matter:

- **Documented PTC Mathcad behavior**;
- **Inference from documented behavior**; or
- **Repository design decision**.

Where PTC does not publish enough internal engine details, define a compatibility boundary: preserve opaque region/calculation data when the current document architecture can do so, defer the behavior, or require an optional external engine bridge. Do not claim lossless opaque preservation if the current codebase cannot store it.

### 3. Reconcile the target with the current app

Perform an explicit gap analysis before specifying the final architecture. Separate:

- capabilities already present and reusable;
- small extensions that naturally fit existing abstractions;
- prerequisite subsystems that do not yet exist;
- changes with a wide exhaustive-match or file-format blast radius;
- optional performance infrastructure that should be added only after profiling; and
- behavior that must be deferred without compromising persisted data.

Do not make the first implementation depend on unrelated future systems. Define:

1. a **baseline release** that is useful, Mathcad-aligned, testable, and can be folded into the current repository; and
2. **compatibility-expansion milestones** that reach complete Mathcad behavior after prerequisites exist.

The baseline must never silently flatten, approximate, discard, or reinterpret semantic math or units merely to avoid implementing them. Unsupported operations must preserve data, provide a documented diagnostic badge, or refuse clearly.

Use existing standard-library collections and repository mechanisms by default. Recommend new crates or packages only when the live code lacks a suitable mechanism and the benefit is concrete. For every proposed dependency, state:

- which module/crate owns it;
- why the standard library/current code is insufficient;
- license and platform considerations;
- determinism and numerical stability implications; and
- whether it is baseline-required or profiling-gated.

Do not assume dependency graphs, parallel calculation schedulers, symbolic algebra engines, unit dimensional analysis solvers, or rich plotting pipelines exist. Verify each one. If absent, either specify the smallest repository-native addition or defer it.

## Mandatory specification structure

Use the following structure, adding subsections whenever the feature requires them. Do not omit a section merely because it is difficult; state “not applicable” with justification or define the staged boundary.

### Title, status, scope, and executive summary

- Name the Mathcad feature and the repository modules/crates affected.
- Define semantic, behavioral, `.mcdx`, evaluation, rendering, and interaction compatibility promises.
- State explicit non-goals.
- Summarize baseline versus full compatibility.

### PTC Mathcad calculation, unit, and behavior grounding

- Explain the official Mathcad object model, region flow, and terminology.
- Provide precise syntax, token, unit-dimension, or schema tables where applicable.
- Explain scope, variable precedence, and execution order.
- Distinguish authoritative persisted state from derived evaluation/cache/display state.
- Explain `.mcdx` version constraints and engine boundaries.
- Cite official PTC documentation throughout.

### Current repository fit and gap analysis

- Describe the exact existing types, modules, evaluation paths, file paths, and renderer paths that will be extended.
- Identify missing prerequisites and dependency-direction hazards.
- Identify every relevant exhaustive region/token match and give it an intentional behavior.
- State the current native format version discovered from code and whether this feature requires a version bump. Never hardcode the version from this prompt.
- Explain how existing user work and dirty-tree files were treated during analysis.

### 1. Data models and memory architecture

- Provide concrete data structures, enums, identifiers, flags, DTO boundaries, and collection choices.
- Ensure code snippets respect actual dependencies and can plausibly compile in this workspace.
- Keep file format/serialization types separate from UI types and pure evaluation/math types.
- Specify ownership, identity, lifetimes, cross-references, deletion, cloning, and undo implications.
- Avoid self-referential structures and unnecessary heap allocation in evaluation hot loops.
- Define authoritative versus derived/cache data.
- Define validation invariants and document validation additions.
- Include a dependency policy and distinguish baseline dependencies from later optimizations.

### 2. Core algorithms and mathematical/evaluation pipeline

- Give ordered, implementable algorithms rather than high-level labels.
- Define dimensional analysis rules, unit conversion matrices, numeric tolerances, symbolic evaluation routines, and numerical predicates.
- Cover parsing, type-checking, dimensional validation, evaluation, editing, cloning, transformation, deletion, and recalculation scheduling.
- Provide formulas, pseudocode, complexity, scratch-memory strategy, cancellation/resource checks, and deterministic tie-breaking where relevant.
- Explain how high-precision arithmetic, underflow/overflow, and non-convergent solve blocks are handled.
- Separate exact semantic mathematics from view-dependent formatting.

### 3. Rendering, plotting, layout, and spatial strategy

- Fit the design to the renderer and layout engine that actually exists.
- Specify region bounds, culling, paint phases, selection highlighting, error badging, plot rendering, and pagination behavior.
- Define cache ownership and invalidation using mechanisms the repository already has.
- Ensure proper separation of concerns between layout calculation and UI event handling.
- Make synchronous bounded behavior the baseline unless asynchronous evaluation workers already exist for this class of computation.
- Treat advanced dependency graphs, worker pools, LRU calculation caches, and GPU rendering as measured later optimizations unless clearly necessary.
- State density, memory, and time guards.

### 4. Interaction and region/command lifecycle

- Define a state machine for the baseline command and the complete Mathcad interaction.
- Cover region creation, math typing, unit placeholder insertion, evaluation triggers, Enter/Escape, Undo within edits, previews, acceptance, cancellation, and contextual properties/ribbon UI.
- Define edit commands, properties inspection, and keyboard navigation.
- Ensure document changes occur through atomic actions and one logical operation corresponds to one transaction/undo step.
- State where user settings, worksheet state, and transient widget/command state live.
- Include an operation compatibility matrix covering existing repository commands such as region select, copy, move, delete, group, collapse, solve block insertion, unit conversion, and plot configuration. Each must say supported, deferred, or explicitly refused.

### 5. Integration with existing Mathcad-aligned subsystems

Address every applicable subsystem:

- math styles, text styles, color palettes, and formatting rules;
- unit systems (SI, USCS, CGS, custom user-defined units) and dimensional consistency checks;
- worksheet layout, headers, footers, pages, and printing/export;
- variable scoping, global definitions (`≡`), local assignments (`:=`), and range variables;
- solve blocks (`Given`, `Find`, `Minerr`, constraints, initial guesses);
- symbolics evaluation and transformation rules;
- atomic undo/redo and dependency-based recalculation propagation;
- native `.mcdx` schema/versioning, XML parsing, and migration;
- file read/write routing, version selection, DTO parsing, warnings, and independent-reader tests; and
- optional external calculation engine integration.

For a missing subsystem, mark the integration as later-phase and explain how imported data remains safe in the meantime. Do not create fake partial infrastructure.

### 6. Error handling, corrupt input, and edge cases

- Define a concrete error taxonomy compatible with current dependency policy.
- Separate parse, dimensional mismatch, non-convergence, runtime evaluation, command, and rendering failures.
- Define behavior for corrupt/truncated `.mcdx` XML structures, cyclic variable dependencies, division by zero, undefined variables, unsupported functions, nonfinite numbers, overflow, allocation failure, and malformed expressions.
- Define safe parser/evaluator limits and when they can actually be enforced with the current parsing architecture.
- Ensure no panic, hang, infinite loop, or unbounded allocation is possible from hostile files.
- Define user-facing diagnostics, error badges, and recovery without data loss.

### Module decomposition and repository change map

- List the exact proposed files by module/crate and responsibility.
- Respect production file-size limits and do not grow files already exempted as too large.
- Identify existing files that need exhaustive-match changes.
- List additions to commands, shortcuts, UI panels, properties, native files, test fixtures, `ARCHITECTURE.md`, and `ui_plan.md`.
- Do not claim a file or API exists without verifying it.

### Staged implementation plan

- Start with a compile-surface/exhaustive-match audit.
- Provide small milestones with prerequisites, deliverables, tests, and exit criteria.
- Identify the exact milestone that produces the first shippable baseline.
- Put advanced symbolics, complex solve-block algorithms, performance infrastructure, and unavailable subsystem integration in later milestones where appropriate.
- Do not defer semantic file round-trip until the very end.

### Verification and acceptance criteria

- Define unit, property, fuzz, evaluation runner, undo/redo, native round-trip, `.mcdx` golden-fixture, independent-parser, rendering, plot, and Mathcad conformance tests.
- Identify which tests gate the baseline and which gate later milestones.
- Include corrupt/adversarial fixtures.
- Include measurable performance and memory budgets appropriate to the current architecture.
- End with separate **Baseline Definition of Done** and **Full Compatibility Definition of Done** checklists.
- Require the repository's actual formatting, linting, and workspace test commands discovered during inspection.

### Official references

- Provide a curated list of the official PTC Mathcad pages actually used.
- Keep citations near claims as well as in this final list.
- Do not cite search pages or fabricate titles/URLs.

## Quality bar

Be exhaustive but precise. Prefer exact tables, invariants, algorithms, state transitions, type definitions, and file-level integration instructions over broad prose. Explain tradeoffs and compatibility boundaries explicitly. Do not hide uncertainty: research it, label it, or defer it safely.

The specification must be internally consistent. Before saving it, perform a final audit for:

- stale repository facts;
- unresolved placeholders other than the configuration value in this template;
- crate/module dependency violations;
- file-format types leaking into core evaluation logic;
- UI types leaking into mathematical engines or commands;
- invented revisions, caches, indexes, workers, tables, or dictionaries;
- use of unsupported Mathcad terminology;
- claims of undocumented calculation engine behavior;
- baseline requirements that depend on absent future subsystems;
- missing exhaustive-match behavior;
- incompatible dependency recommendations;
- missing undo/redo and file-version implications;
- unbounded parsing/evaluation paths;
- contradictory acceptance criteria; and
- unbalanced Markdown fences or malformed links.

Write the completed specification to the derived path in `private/`. In the final response, report the saved path, the baseline scope, the principal deferred compatibility items, and any Mathcad behavior that remains undocumented. Do not implement the feature unless separately asked.
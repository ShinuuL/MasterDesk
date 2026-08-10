# AGENTS.md — MasterDesk Development Instructions

## 1. Project

You are working on **MasterDesk**, a desktop-first notes and task management application.

MasterDesk is inspired by Sticky Notes but must be significantly more extensible.

The application will eventually integrate with the Mastersys customer-support system, provide mobile visualization/access and offer AI-assisted recommendations.

---

# 2. Non-Negotiable Rules

## Rule 1 — Never Guess

Never invent:

* APIs.
* Endpoints.
* Authentication flows.
* Mastersys behavior.
* Library capabilities.
* OS capabilities.
* AI provider behavior.
* Database contracts.
* Security mechanisms.

If something is unknown and materially affects implementation:

**STOP and ask the DEV.**

---

## Rule 2 — Research Before Implementing

Before introducing a library, framework, SDK or important implementation technique:

1. Research it.
2. Prefer official documentation.
3. Use available MCPs.
4. Verify current maintenance status.
5. Verify licensing.
6. Verify OS compatibility.
7. Check known limitations.
8. Compare reasonable alternatives.
9. Document the decision when architecturally relevant.

Do not install dependencies simply because they are popular.

---

# 3. Technology Direction

The current preferred hypothesis is:

```text
Rust
+
Tauri 2
+
TypeScript
```

This is not an immutable decision.

Before starting substantial implementation, evaluate:

* Tauri.
* Slint.
* egui.
* Iced.
* TypeScript UI alternatives.
* Angular.
* Python desktop alternatives.

The final technology decision must be documented in an ADR.

Tauri is currently an especially strong candidate because its architecture supports Rust application logic with a web frontend and targets desktop and mobile platforms.

Slint must also be considered because it provides Rust integrations and cross-platform desktop/mobile capabilities.

---

# 4. Architecture

Maintain a strict separation between:

```text
UI
 ↓
Application
 ↓
Domain
 ↓
Interfaces / Ports
 ↓
Infrastructure / Integrations
```

Never make the domain depend directly on:

* Mastersys.
* AI providers.
* HTTP clients.
* Database implementations.
* OS-specific APIs.
* UI framework details.

Use abstractions such as:

```text
NoteRepository
TaskRepository
NotificationService
WindowService
AuthenticationProvider
SupportSystemProvider
AIProvider
```

---

# 5. MasterDesk Domain

The core application must work independently of Mastersys.

Notes must not require a Mastersys ticket.

Tasks must not require a Mastersys ticket.

External integrations should be represented through adapters.

Conceptually:

```text
MasterDesk
   |
   +-- Local tasks
   |
   +-- Mastersys tasks
   |
   +-- Future integrations
```

---

# 6. Notes

Notes should support, where implemented:

* Title.
* Content.
* Tags.
* Priority.
* Deadline.
* Reminder.
* Completion.
* Color.
* Theme.
* Size.
* Position.
* Opacity.
* Pinning.
* Always-on-top.
* Archive.
* Delete.

All reasonable visual behavior should be configurable.

---

# 7. Always-on-Top

Always-on-top functionality is an OS integration.

Do not assume it works identically on Windows, macOS and Linux.

Before implementation:

* Research the selected framework.
* Research native OS behavior.
* Test on supported operating systems.
* Document limitations.

Do not fake the behavior with unreliable workarounds.

---

# 8. Notifications

Deadline reminders are a core feature.

Users must eventually be able to configure reminder thresholds.

Examples:

```text
5m
10m
15m
30m
1h
2h
custom
```

Notifications may eventually include:

* System notification.
* Sound.
* Visual emphasis.
* Repetition.
* Snooze.

Research the appropriate implementation before coding.

---

# 9. UI Customization

Customization is a core requirement.

Avoid hard-coded UI behavior.

Users should eventually be able to customize:

* Theme.
* Colors.
* Fonts.
* Font sizes.
* Opacities.
* Dimensions.
* Rounded corners.
* Shadows.
* Notification behavior.
* Keyboard shortcuts.
* Startup behavior.
* Language.
* Date/time formats.

---

# 10. Mastersys

Mastersys is an external integration.

Do not couple the entire application to it.

Use:

```text
SupportSystemProvider
```

with a future implementation such as:

```text
MastersysProvider
```

Never implement a Mastersys API call until the real API contract has been validated.

If the API documentation is unavailable:

**Ask the DEV.**

---

# 11. Authentication

Authentication must be abstracted.

Do not assume the future Mastersys authentication mechanism.

Never store plaintext passwords.

Never hard-code tokens.

Never commit credentials.

Use secure credential storage where appropriate.

---

# 12. AI

AI will eventually analyze tasks/tickets approaching expiration.

The intended role of AI is advisory.

AI may eventually:

* Read authorized task context.
* Identify relevant information.
* Suggest next actions.
* Suggest possible ways to complete a task.
* Prioritize possible actions.

AI must not automatically perform external side effects unless explicitly authorized by a future feature.

Do not let AI directly:

* Close tickets.
* Send messages.
* Modify support records.
* Delete records.
* Change permissions.

without explicit authorization and validation.

---

# 13. AI API Key

Prepare the architecture for an AI API key.

Use configuration such as:

```text
MASTERDESK_AI_API_KEY
```

Never commit a real key.

Never place secrets in source code.

Never put secrets in test fixtures.

Never log secrets.

The exact AI provider and secret-management strategy must be researched before implementation.

---

# 14. Mobile

Desktop is the primary product.

Mobile is initially for visualization/access.

Do not assume full feature parity.

Keep the architecture capable of supporting:

```text
Desktop
   |
Shared API/domain
   |
Mobile/Web
```

The mobile strategy must be decided after framework research.

---

# 15. MCP Usage

Use available MCPs before implementing:

* UI libraries.
* Frameworks.
* APIs.
* SDKs.
* External integrations.
* Important OS features.
* Database libraries.
* Authentication libraries.
* AI libraries.

Research must be based on current information.

When an MCP is available and relevant, use it instead of relying on memory.

---

# 16. Dependency Policy

For every significant dependency document:

```text
Name
Version
Purpose
Official documentation
License
OS support
Maintenance status
Known limitations
Alternatives
Reason for selection
```

Avoid unnecessary dependencies.

Prefer stable, well-maintained and well-documented libraries.

---

# 17. Error Handling

Never silently swallow errors.

Errors should:

* Be handled at the appropriate boundary.
* Provide useful context.
* Avoid leaking secrets.
* Be logged appropriately.
* Be understandable to users when surfaced in the UI.

---

# 18. Security

Assume MasterDesk may eventually handle sensitive customer-support information.

Therefore:

* Validate external input.
* Minimize stored data.
* Minimize permissions.
* Protect credentials.
* Avoid sensitive logs.
* Secure integration boundaries.
* Treat AI responses as untrusted.
* Do not trust external APIs blindly.
* Do not execute AI-generated commands automatically.

---

# 19. Testing

Add tests for business-critical behavior.

At minimum:

```text
Deadline calculations
Reminder calculations
Note state
Task state
Persistence
Configuration
Integration boundaries
```

Cross-platform features must be tested on the platforms they claim to support.

Compilation alone is not cross-platform validation.

---

# 20. Code Quality

Prefer:

* Small modules.
* Explicit interfaces.
* Strong types.
* Dependency inversion.
* Clear naming.
* Small functions.
* Testable business logic.
* Documentation for non-obvious decisions.

Avoid:

* Giant files.
* Giant components.
* Global mutable state.
* Hidden side effects.
* Magic values.
* Hard-coded configuration.
* Temporary hacks without documentation.

---

# 21. ADRs

Create ADRs for significant decisions.

Examples:

```text
ADR-001 — Desktop framework
ADR-002 — UI framework
ADR-003 — Persistence
ADR-004 — Notifications
ADR-005 — Authentication
ADR-006 — Mastersys integration
ADR-007 — AI architecture
ADR-008 — Mobile architecture
```

Each ADR should contain:

```text
Context
Options
Decision
Consequences
```

---

# 22. DEV Escalation

Ask the DEV instead of guessing when:

* Requirements are ambiguous.
* External API information is missing.
* Security behavior is unclear.
* A library capability cannot be verified.
* OS behavior is uncertain.
* A destructive action is involved.
* An integration contract is unknown.
* AI could produce an external side effect.
* Multiple valid architectures have materially different consequences.

Use this format:

```text
What is known:
What is unknown:
Why it matters:
Options:
Recommendation:
Decision required:
```

---

# 23. Definition of Done

Do not consider a feature complete merely because it compiles.

A feature is complete when:

* Requirements are satisfied.
* Dependencies have been researched.
* Relevant MCP research was performed.
* Architecture boundaries are respected.
* Tests exist where appropriate.
* Errors are handled.
* Security was considered.
* OS behavior was validated.
* Documentation was updated.
* No APIs or capabilities were invented.
* No unresolved assumptions remain.

---

# 24. Development Order

Prefer this sequence:

```text
1. Research
2. Architecture decision
3. ADR
4. Minimal implementation
5. Tests
6. Platform validation
7. Documentation
8. Refactoring
```

Do not start with complex Mastersys or AI integrations.

Build a strong local MasterDesk core first.

---

# 25. Golden Rule

> When you don't know, don't invent. Research it or ask the DEV.

MasterDesk must remain extensible, secure, cross-platform and independent of any single external integration.

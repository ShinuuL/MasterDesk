# MasterDesk — Project Artifacts

## 1. Project Identity

**Project name:** MasterDesk

**Initial product type:** Desktop-first productivity application with future mobile visualization and integration with the Mastersys customer-support system.

**Primary purpose:**
Provide a customizable notes/task workspace inspired by Sticky Notes, but designed to associate notes with support tickets, tasks, deadlines, reminders and future AI-assisted recommendations.

---

# 2. Product Vision

MasterDesk is a desktop application designed for users who need persistent, visible and actionable notes while working across multiple applications.

The application must allow the user to:

* Create notes quickly.
* Keep notes visible above other applications.
* Pin/unpin notes.
* Configure colors, sizes, opacity, typography and visual behavior.
* Associate notes with tasks or customer-support tickets.
* Define deadlines.
* Configure reminder thresholds.
* Receive notifications when a task/ticket is approaching expiration.
* Organize notes independently of any future Mastersys integration.
* Eventually synchronize selected data with Mastersys.
* Eventually visualize relevant information from mobile devices.
* Eventually use AI to analyze tasks/tickets and suggest possible actions before deadlines expire.

MasterDesk must **not be designed as a Mastersys-only application**.

The core domain must remain independent from external support systems.

---

# 3. Initial Functional Scope

## 3.1 Notes

Each note should support, at minimum:

* Title.
* Rich or structured text.
* Creation date.
* Last modified date.
* Optional deadline.
* Optional reminder threshold.
* Optional task/ticket reference.
* Optional tags.
* Color/theme.
* Size.
* Position.
* Opacity.
* Always-on-top behavior.
* Pin/unpin.
* Archive.
* Delete.
* Restore, if soft-delete is implemented.
* Favorite/important status.
* Completion status.
* Optional priority.
* Optional attachments/links in a future version.

Notes must work without any external support-system integration.

---

# 4. Ticket/Task Abstraction

Do not create the domain model around "Mastersys ticket".

Use a generic abstraction such as:

```text
Task
 ├── id
 ├── title
 ├── description
 ├── status
 ├── priority
 ├── due_at
 ├── reminder_at
 ├── source
 ├── external_id
 └── metadata
```

Possible `source` values may eventually include:

```text
local
mastersys
other
```

The exact model must be validated before implementation.

The architecture must allow additional integrations without rewriting the note system.

---

# 5. Deadline and Reminder System

The user must be able to configure how early MasterDesk warns them.

Examples:

```text
5 minutes before
10 minutes before
15 minutes before
30 minutes before
1 hour before
2 hours before
Custom duration
```

The system should eventually support:

* Desktop notification.
* Sound.
* Visual emphasis of the note.
* Optional flashing/highlighting.
* Optional notification repetition.
* User-configurable behavior.
* Per-note configuration.
* Global default configuration.

Do not assume a specific notification library.

The development agent MUST research and validate the appropriate notification mechanism for each supported operating system before implementation.

---

# 6. Always-on-Top Notes

Pinned notes must be capable of remaining visible above other applications.

The implementation must consider:

* Windows.
* macOS.
* Linux.
* Multiple monitors.
* Window movement.
* Window resizing.
* Application restart.
* Persisted position.
* Persisted size.
* User-configurable opacity.
* Focus behavior.
* Minimize/maximize behavior.
* Full-screen applications.
* OS-specific limitations.

Do not assume that "always on top" behaves identically on all operating systems.

Before implementation, research the native/window-management capabilities of the selected framework and validate them with the available MCPs.

If an operating system has limitations, document them instead of creating an unverified workaround.

---

# 7. User Customization

Customization is a core requirement, not a future enhancement.

The UI should eventually allow configuration of:

* Theme.
* Light/dark mode.
* Colors.
* Font.
* Font size.
* Note dimensions.
* Opacity.
* Border radius.
* Shadows.
* Always-on-top behavior.
* Default reminder time.
* Notification sound.
* Notification behavior.
* Language.
* Date/time format.
* Startup behavior.
* Auto-start.
* Minimize-to-tray behavior.
* Keyboard shortcuts.
* Accessibility preferences.

Avoid hard-coding visual behavior that should reasonably be user-configurable.

---

# 8. Login and Future Mastersys Integration

MasterDesk will eventually contain authentication.

The initial architecture must reserve a dedicated authentication boundary.

Example conceptual structure:

```text
AuthProvider
 ├── LocalAuth / DevelopmentAuth
 └── MastersysAuth
```

The exact authentication mechanism must be researched and validated before implementation.

Do not invent:

* Mastersys APIs.
* Authentication endpoints.
* Token formats.
* API contracts.
* Database schemas.
* Permission models.

If information about Mastersys is unavailable or ambiguous, stop and ask the DEV rather than guessing.

---

# 9. Mastersys Integration

The future Mastersys integration must be isolated behind an adapter/interface.

Conceptually:

```text
SupportSystemProvider
        |
        +-- MastersysProvider
        |
        +-- FutureProvider
```

The core application must not depend directly on Mastersys-specific implementation details.

The integration should eventually support operations such as:

* Authenticate.
* Fetch tickets.
* Fetch tasks.
* Fetch ticket details.
* Update ticket/task status, only if explicitly authorized.
* Associate a MasterDesk note with a ticket.
* Retrieve deadlines.
* Retrieve customer information, only when required and authorized.

Every operation must be validated against the real Mastersys API before implementation.

---

# 10. Future AI Integration

MasterDesk will eventually provide AI-assisted task analysis.

The intended behavior is:

1. User has a task/ticket.
2. Task/ticket approaches its configured deadline.
3. MasterDesk identifies that the task is approaching expiration.
4. AI receives the minimum authorized context.
5. AI analyzes the task/ticket.
6. AI suggests possible next actions.
7. User reviews the suggestions.
8. User decides what to do.

AI must initially be **advisory only**.

The AI must not automatically:

* Close tickets.
* Send customer messages.
* Modify support records.
* Delete information.
* Change ticket status.
* Execute external actions.

unless a future explicit feature authorizes and validates such behavior.

---

# 11. AI API Key

The application must be prepared for an AI API key without hard-coding secrets.

Never commit:

```text
API_KEY=actual-secret
```

Use environment variables or an appropriate secure credential mechanism.

Initial conceptual configuration:

```text
MASTERDESK_AI_API_KEY
```

The exact secret-management approach must be researched and selected before production implementation.

Do not place real API keys in:

* Git.
* Source code.
* Documentation.
* Screenshots.
* Test fixtures.
* Example configuration files.

A safe example may use:

```text
MASTERDESK_AI_API_KEY=<configure-me>
```

---

# 12. Mobile

MasterDesk is desktop-first.

Mobile support initially means **visualization/access to relevant information**, not necessarily full desktop feature parity.

The architecture should allow:

```text
Desktop Application
       |
       +---- Shared Domain/API
       |
       +---- Mobile/Web Client
```

The agent must investigate whether the selected technology can provide an acceptable mobile experience.

Candidate strategies include:

* Shared UI framework.
* Mobile companion application.
* Responsive web application.
* Tauri mobile.
* Separate mobile frontend consuming the same API.

Do not choose one without validating the trade-offs.

---

# 13. Technology Research Requirement

The initial requested technology candidates are:

* Rust.
* Python.
* TypeScript.
* Angular.

Angular is a frontend framework rather than a programming language and must be evaluated as such.

The agent must compare at least:

### Rust

Candidates may include:

* Tauri.
* Slint.
* egui.
* Iced.
* Other actively maintained Rust GUI technologies discovered during research.

### TypeScript

Candidates may include:

* Tauri frontend.
* React.
* Vue.
* Svelte.
* Other suitable frameworks.

### Angular

Evaluate whether Angular adds meaningful value for this project versus a lighter frontend.

### Python

Evaluate:

* Packaging.
* Desktop GUI quality.
* Native OS integration.
* Performance.
* Distribution.
* Long-term maintainability.
* Mobile strategy.

---

# 14. Preliminary Technology Hypothesis

The current preferred direction is:

```text
Rust
  +
Tauri 2
  +
TypeScript
```

Reason:

* Rust can provide the application/core layer.
* Tauri supports desktop applications and mobile targets.
* TypeScript provides a mature UI ecosystem.
* The architecture can expose native functionality through Rust.
* The stack is suitable for a desktop-first product that may later require mobile access.

This is a **hypothesis, not an immutable decision**.

The agent must validate it before implementation.

Slint is an important alternative because it supports Rust and cross-platform desktop/mobile development and offers native-style/custom UI capabilities.

The final decision must be recorded as an ADR.

---

# 15. Mandatory Research Before Implementation

Before implementing any non-trivial feature, the agent MUST research:

* Framework.
* Library.
* UI component.
* OS integration.
* Notification mechanism.
* Window management.
* Persistence layer.
* Authentication library.
* HTTP client.
* Serialization library.
* Encryption/security mechanism.
* AI SDK.
* Mobile strategy.
* Packaging/distribution mechanism.

Research must prioritize:

1. Official documentation.
2. Official repositories.
3. Current maintenance status.
4. Compatibility with supported operating systems.
5. License.
6. Security considerations.
7. API stability.
8. Community/production adoption where relevant.

Use the MCPs available to the agent whenever possible.

---

# 16. MCP Validation Rule

Before adding a dependency, the agent should investigate it through the available MCPs.

The agent should document:

```text
Dependency:
Version:
Purpose:
Official documentation:
License:
Supported OS:
Maintenance status:
Known limitations:
Reason for adoption:
Alternatives considered:
```

Do not add libraries merely because they are popular.

---

# 17. Unverified Functionality Rule

The agent MUST NEVER implement functionality based on assumptions when the behavior affects:

* Operating-system integration.
* Authentication.
* Security.
* Notifications.
* Window management.
* External APIs.
* Mastersys integration.
* AI behavior.
* Data synchronization.
* Data deletion.
* User permissions.

If uncertain:

```text
STOP → DOCUMENT THE UNCERTAINTY → ASK THE DEV
```

Do not invent an API, endpoint, library capability or integration contract.

---

# 18. Development Philosophy

Prefer:

* Small increments.
* Strong typing.
* Explicit interfaces.
* Testable business logic.
* Separation of domain and infrastructure.
* Dependency inversion.
* OS-specific adapters.
* Integration adapters.
* Secure configuration.
* Observable errors.
* Clear documentation.

Avoid:

* Giant components.
* Hidden global state.
* Hard-coded configuration.
* Direct coupling to Mastersys.
* Direct coupling to a specific AI provider.
* Unvalidated dependencies.
* Magic behavior.
* Silent failures.
* "Temporary" hacks without documentation.

---

# 19. Suggested Architecture

The final architecture must be validated by the agent, but the project should aim for a structure similar to:

```text
MasterDesk/
├── apps/
│   ├── desktop/
│   └── mobile/              # future / optional
│
├── core/
│   ├── domain/
│   ├── application/
│   ├── infrastructure/
│   └── integrations/
│
├── integrations/
│   ├── mastersys/
│   └── ai/
│
├── ui/
│   ├── components/
│   ├── screens/
│   ├── themes/
│   └── settings/
│
├── tests/
│
├── docs/
│
├── artifacts.md
├── AGENTS.md
└── CLAUDE.md
```

This structure is conceptual and may be changed after framework research.

---

# 20. Core Architectural Boundaries

The project should maintain these boundaries:

```text
UI
 ↓
Application Services
 ↓
Domain
 ↓
Ports / Interfaces
 ↓
Infrastructure / Integrations
```

Examples:

```text
NoteRepository
TaskRepository
NotificationService
WindowService
AuthenticationProvider
SupportSystemProvider
AIProvider
```

Implementations should live outside the domain layer.

---

# 21. Data Persistence

The application should work locally without Mastersys.

The persistence layer should support:

* Notes.
* Tasks.
* Settings.
* Reminder configuration.
* Window state.
* User preferences.
* Authentication state, using secure storage where appropriate.
* Integration metadata.

The agent must research the best embedded database/storage solution for the selected stack.

Possible candidates may include SQLite or another embedded store, but no technology is pre-approved.

---

# 22. Offline-first Principle

Basic note functionality should not depend on internet connectivity.

The user must be able to:

* Create notes.
* Edit notes.
* Delete/archive notes.
* Configure notes.
* Set reminders.
* View existing local data.

External integrations may require connectivity.

---

# 23. Security Principles

Security is mandatory because future versions may contain customer-support information.

Requirements include:

* Never hard-code secrets.
* Minimize stored credentials.
* Use secure OS credential storage where appropriate.
* Validate external data.
* Avoid logging secrets.
* Avoid logging customer-sensitive information unnecessarily.
* Apply least privilege.
* Separate authentication from authorization.
* Validate external API responses.
* Treat AI input/output as untrusted data.
* Never allow AI output to directly execute privileged actions.

---

# 24. Testing

At minimum, the project should eventually contain:

### Unit tests

For:

* Deadline calculation.
* Reminder calculation.
* Note state.
* Task state.
* Configuration.
* Domain rules.

### Integration tests

For:

* Persistence.
* Notifications.
* Authentication adapters.
* Mastersys adapter.
* AI adapter.

### Platform tests

At minimum:

```text
Windows
macOS
Linux
```

Mobile tests should be added when mobile support is implemented.

---

# 25. Cross-platform Acceptance Criteria

Before declaring a desktop feature complete, verify it on the supported operating systems.

Do not claim:

```text
Cross-platform compatible
```

based solely on compilation.

The agent should validate actual behavior.

---

# 26. UX Requirements

The application should feel fast and unobtrusive.

The user should be able to create a note quickly.

Potential future shortcuts:

```text
New note
Pin note
Search notes
Complete task
Snooze reminder
Open related ticket
```

Keyboard shortcuts must be configurable.

The UI should support accessibility where the selected framework permits it.

---

# 27. Observability

The application should have structured logging.

Logs should distinguish:

```text
DEBUG
INFO
WARN
ERROR
```

Sensitive information must not be logged.

Errors from integrations should be actionable and understandable.

---

# 28. ADR Requirement

Important technical decisions must be documented.

Examples:

```text
ADR-001 — Desktop framework
ADR-002 — UI framework
ADR-003 — Persistence
ADR-004 — Notification system
ADR-005 — Authentication
ADR-006 — Mastersys integration architecture
ADR-007 — AI provider architecture
ADR-008 — Mobile strategy
```

Every ADR should explain:

* Context.
* Options considered.
* Decision.
* Consequences.
* Reversal cost.

---

# 29. Initial Milestones

## Phase 0 — Research

* Evaluate Rust.
* Evaluate Tauri.
* Evaluate Slint.
* Evaluate alternative Rust GUI frameworks.
* Evaluate TypeScript UI options.
* Evaluate Angular.
* Evaluate Python.
* Evaluate desktop OS support.
* Evaluate mobile strategy.
* Record ADR.

## Phase 1 — Foundation

* Create MasterDesk project.
* Configure repository.
* Configure CI.
* Establish architecture.
* Establish linting/formatting.
* Establish testing.
* Establish configuration management.

## Phase 2 — Local Notes

* Create note.
* Edit note.
* Delete/archive.
* Pin.
* Always-on-top.
* Position persistence.
* Size persistence.
* Customization.

## Phase 3 — Tasks and Deadlines

* Task abstraction.
* Deadline.
* Reminder.
* Notifications.
* Snooze.
* Completion.

## Phase 4 — Authentication

* Authentication abstraction.
* Secure session handling.
* Login UI.
* Prepare Mastersys authentication adapter.

## Phase 5 — Mastersys

Only after the real Mastersys API is validated.

## Phase 6 — AI

Only after the AI provider and security model are validated.

## Phase 7 — Mobile

Determine the best mobile delivery strategy based on the chosen architecture.

---

# 30. Definition of Done

A feature is not considered complete merely because it compiles.

A feature is complete when:

* Requirements are understood.
* Dependencies were researched.
* MCP research was performed where applicable.
* Architecture boundaries are respected.
* Tests exist where appropriate.
* Error handling exists.
* Security implications were considered.
* Supported OS behavior was validated.
* Documentation was updated.
* No unverified API or integration was invented.
* The DEV's explicit requirements are satisfied.

---

# 31. DEV Escalation Policy

Ask the DEV when:

* Requirements conflict.
* An external API is ambiguous.
* Mastersys behavior is undocumented.
* A security decision is unclear.
* Two frameworks have materially different architectural consequences.
* A feature requires an unverified OS capability.
* A dependency is questionable.
* A destructive action is being considered.
* AI behavior could create an external side effect.
* There is insufficient information to safely implement a feature.

When asking the DEV, clearly state:

```text
What is known:
What is unknown:
Why it matters:
Options:
Recommended option:
Decision required:
```

---

# 32. Project Principle

> MasterDesk should be extensible without becoming coupled.

The desktop application comes first.

Mastersys is an integration.

AI is an integration.

Mobile is a client.

The note/task domain belongs to MasterDesk.

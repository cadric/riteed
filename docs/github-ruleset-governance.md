# GitHub Ruleset Governance

This file records the completed `RIT-AUD-017` ruleset remediation and the
completed `RIT-GEN-038` transition. The 2026-05 activation evidence and the P3
pre-activation design remain below as historical records; their open-state and
pending-action statements no longer describe the remote repository.

## RIT-GEN-038 closure evidence, 2026-09-06

The owner-approved activation completed with the governance credential present
only as the `ruleset-governance-live` environment secret, updated at
`2026-09-06T15:42:03Z`. Environment policy `59257399` admits exactly `main`.
All eight owner-side read-permission probes passed before migration. The
repository secret was deleted and read back with count zero; no secret value
was read or recorded.

Ruleset `16713108` changed exactly one required context,
`ruleset-governance` to `governance-static`, using reviewed before, after and
inverse payloads. The other five contexts, conditions, deletion,
non-fast-forward, signed-commit and pull-request rules and reviewed bypass
actor were preserved. Version-tag ruleset `16713116` remained active with no
bypass actors.

PR #38 head `5d7218c0963b1373e737f2595a1e964a8c0eebe4` passed all six
required contexts and CodeQL, with live governance correctly skipped and
static checkout
`83de745620b064ca121e5a4fe063daec760649b5`. It merged normally at
`2026-09-06T15:44:43Z` as signed commit
`28d754729ae575e0078804e379bb29e1110785e0`; its tree matches the tested PR
head. Main Validate run `34043264885` and CodeQL run `34043264871` completed
successfully. Static job `101513732735` and live job `101513732729` both used
that exact commit; the live job's identity assertion and unique decisive
governance step succeeded.

The release evidence collector and checker accepted the same exact main SHA.
This proves publish-check eligibility only; no tag, release, signing operation
or dependency merge occurred. Dependabot PR #39 independently exercised the
PR boundary after rebasing onto that main commit: Validate `34043507117`,
CodeQL `34043507114` and all six required contexts succeeded for head
`a207805fca54738cfcab46ed072807cfa9daabe8`; live governance was deliberately
skipped. Its actual synthetic-merge checkout
`c236cd02300717b9c31d668e66358a585c79cd12` combined that head with exact main;
its identity assertion and static gate succeeded. PR #39 remains open and
unmerged.

These terminal, provenance-bound ordinary-PR, protected-main, publish-check and
Dependabot results satisfy the typed removal condition. Enforcement remains in
place; only `POLICY-RIT-GEN-038` is removed from planned remediation.

## Historical RIT-GEN-038 P3 pre-activation design, 2026-09-06

The text through “Evidence still required for closure” is retained as the
pre-activation plan and evidence snapshot. The closure record above supersedes
its open-state and future-action language.

This section began as the mandatory Task 8A read-only design/evidence record.
On 2026-09-06 the repository owner explicitly approved the bounded GitHub
activation: publish the reviewed local commits through a PR/merge, create the
main-only governance environment, move the governance credential to it, and
replace exactly the old required context. That approval does not cover other
rules, signing environments or releases. RIT-GEN-038 remains open under the
typed remediation in `policy/release.policy.json` until Task 8B activation and
real post-activation evidence are complete.

### Observed remote state

Read-only refresh on 2026-09-06 found remote `main` at
`1c7b81c7bc462acf29ca92f8394b1ecff5603049`. Local integrated `main` is 15
commits ahead, so a P2/P3 publication decision must account for those commits
before any candidate PR or merge. `Protect main` ruleset `16713108` is active
and requires exactly:

```text
dependency-preflight
policy-pack
native-tests
ruleset-governance
flatpak-tests
flatpak
```

Its deletion, non-fast-forward, signed-commit and pull-request rules, newer
pull-request parameters, strict required-check policy, and reviewed
`User:964797:pull_request` bypass remain in force. `Protect version tags`
ruleset `16713116` is active with no bypass actors. There is no governance
environment. The only repository Actions secret name is
`RULESET_GOVERNANCE_TOKEN`; no secret value was read or requested.

The current check is not truthful for every event:

| Validate evidence | Check/run `head_sha` | Actual code/policy checkout | Credential and decisive result | Conclusion |
|---|---|---|---|---|
| Ordinary same-repository PR run `31342646721`, job/check `93318869249`, 2026-08-09 | PR head `00067e7edc30061ba74628d5e90d822355fddf25` | Synthetic merge `7938353a6c2b966d5a3bf10a849b14a87cf3adbc` (head into base `0c143768f5a2825a455e5b60aabe85f4303a76e9`) | Repository PAT available; `Verify GitHub ruleset governance` succeeded | Live PR code executed with the repository credential; check SHA and checked-out SHA differ. |
| Dependabot PR run `31343275411`, job/check `93320469610`, 2026-08-10 | PR head `b6776eb33fa20b7a55c46850279fd7eb1cf93b6b` | Synthetic merge `b411527969318d29c8d00ddeb1a7faa497e1e948` (head into base `4470eb7630e955755052f222f5c41eab4946722e`) | Decisive step skipped; enclosing job/check succeeded | Required `ruleset-governance` passed without governance verification. |
| Main push run `31343760989`, job/check `93321778613`, 2026-08-10 | `1c7b81c7bc462acf29ca92f8394b1ecff5603049` | Same exact SHA, confirmed from checkout log | Decisive step succeeded | Representative truthful main execution for the old layout. |
| Schedule run `33484656541`, attempt 1 job/check `99781849987`, attempt 2 job/check `101489287483` | Same recorded main SHA | Same exact SHA, confirmed from checkout log | Attempt 1 returned HTTP 401 from both live API reads; after the owner updated the token, attempt 2 completed and the decisive step succeeded at 2026-09-06 12:49:27–33Z | The 401 is historical and its cause remains unknown. Attempt 2 is the current old-layout baseline, not proof of the new environment boundary. |
| Fork PR | No representative inspected run | Not available | Current condition is designed to skip the credentialed step | Missing remote evidence; must be verified after activation without exposing a credential. |

For `pull_request`, GitHub documents `GITHUB_SHA` as the synthetic merge commit
and `GITHUB_REF` as `refs/pull/<n>/merge`; `github.event.pull_request.head.sha`
is the PR head. The observed checkout logs confirm why check/run `head_sha` and
the code/policy checkout must be recorded separately. A custom environment
branch rule is matched against `GITHUB_REF`, so a branch-only `main` rule does
not admit `refs/pull/*/merge`. See GitHub's
[event reference](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows),
[deployment environment reference](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments),
and [deployment branch-policy API](https://docs.github.com/en/rest/deployments/branch-policies).

### Selected workflow and check layout

The local Task 8B candidate splits the overloaded check name into two meanings:

1. `governance-static` is an unconditional Validate job on every current
   event: ordinary PR, fork/Dependabot PR, push, schedule and manual dispatch.
   It receives no governance environment, custom token, `GH_TOKEN` or
   `GITHUB_TOKEN`. Its checkout must be the event commit and an explicit
   `git rev-parse HEAD == GITHUB_SHA` assertion records the policy revision
   actually tested. On a PR this is the proposed synthetic merge, not merely
   the check-run head SHA. Its one policy step uses exactly:

   ```text
   python3 -m tools.policy_check --release-static-check --root app --strict
   ```

   The mutually exclusive CLI mode resolves the explicit app and contract
   roots and runs `release.check_release` only. It does not run Cargo, GNOME,
   Flatpak, required commands, network calls or token discovery. It is a
   scoped release/workflow shape check, not the full app gate.
   The standard checkout action's read-only event token is not a governance
   credential; no signing secret is present anywhere in PR governance.

2. `governance-live` exists only for integrated-main `push`, `schedule` and
   `workflow_dispatch` runs. Its owning job condition must enumerate those
   events and require `github.ref == 'refs/heads/main'`. It uses the dedicated
   `ruleset-governance-live` environment, explicitly checks out
   `${{ github.sha }}`, and asserts allowed event, exact main ref, repository
   identity and `git rev-parse HEAD == GITHUB_SHA` before invoking the live
   checker. No PR event can enter the environment. The uniquely named
   `Verify GitHub ruleset governance` step must itself finish successfully;
   an aggregate green job with that step missing, skipped, neutral, duplicated
   or failed is not evidence.

The static job validates proposed policy/workflow code. The live job executes
only integrated code and validates repository state. Neither check is an alias
for the other, and the old `ruleset-governance` name is not reused for a
static-only success.

A separate privileged `pull_request_target` or `workflow_run` workflow was
considered but is not selected. It would add a second trusted-code and artifact
provenance boundary without improving the chosen contract: candidate policy is
already checked tokenlessly, and the protected integrated-main job can bind
its own checkout, event and SHA directly. The existing `Validate` main-push
producer is retained, but its live job is admitted only by the main-only
environment and the exact event/ref assertions above.

### Environment and credential boundary

Create `ruleset-governance-live` with
`protected_branches=false`, `custom_branch_policies=true`, and one custom
deployment branch policy named `main` with type `branch`. The owner enters
`RULESET_GOVERNANCE_TOKEN` directly as an environment secret outside chat.
The canonical secret must not remain at repository scope.

The expected fine-grained PAT permissions are repository
`Administration: read`, `Environments: read`, `Secrets: read`, plus implicit
`Metadata: read`. `Secrets: read` is needed to prove through every paginated
page that the repository-level name is absent; `Environments: read` is needed
to list environment secret metadata. Neither API returns secret values. See
GitHub's [Actions secrets API](https://docs.github.com/en/rest/actions/secrets)
and [deployment environments API](https://docs.github.com/en/rest/deployments/environments).

Secret migration is intentionally not represented as fully reversible JSON:
GitHub cannot return the old value. The proposed approval-gated order is to
create/protect the environment, have the owner enter and permission-test the
credential outside chat, verify environment and secret-name metadata, then
delete the repository-level copy. Recovery after deletion requires owner-held
credential re-entry. Do not automatically restore the unsafe repository scope
or delete the protected environment copy.
The first scheduled attempt's HTTP 401 is historical; do not infer expiry,
revocation or a permission cause. The successful rerun proves the updated
repository-scoped credential can execute the old live checker, but it does not
prove the new `Secrets: read` metadata checks or protected environment path.
Activation still requires the owner-side permission proof and admin metadata
before deleting the old name; the actual protected environment job can prove
the new path only after the authorized workflow merge.

### Acceptance matrix

| Evidence state | PR merge eligibility | Publish eligibility |
|---|---|---|
| Static success; no governance credential involved | May satisfy `governance-static` with the other five required PR contexts | Static evidence alone never qualifies publish. |
| Static missing or failed | Blocks a normal PR through GitHub's required-check behavior | Rejected as an incomplete or failed candidate check set. |
| Static skipped or neutral | GitHub can accept these conclusions for a required context; the offline workflow contract must therefore reject any condition, continuation or fallback wiring that permits them | The publish helper explicitly rejects every non-success conclusion. |
| Live token missing or live checker fails | PR remains independent of live credentials | Rejected. |
| Live job green but decisive step missing, skipped, neutral, duplicated or failed | Not a PR context | Rejected even if the aggregate job/check is success. |
| Live success on wrong SHA, branch, event, repository, workflow, job or check producer | Not a PR context | Rejected. |
| Live decisive step completed/success on exact candidate SHA from a policy-owned protected-main `Validate` producer event | Not a PR context | May satisfy `governance-live` after all static/build contexts also succeed. |

The publish collector binds its selected completed/success check run
to all of the following: exact candidate SHA; GitHub Actions app; exact check
name and selected check-run ID; Actions job `check_run_url`; job `html_url`
equal to the check's `details_url`; unique successful decisive step; Actions
run repository and head repository `cadric/riteed`; workflow `Validate` at
`.github/workflows/validate.yml`; event in the policy-owned set `push`,
`schedule`, or `workflow_dispatch`; head branch `main`; and exact run
`head_sha`. Selection keeps newest matching check-run semantics: a newer failed
live check for the SHA cannot fall back to an older success, while a newer
fully valid protected-main schedule or manual run may qualify publish. A real
main-push run remains mandatory activation evidence even though it is not the
only allowed later producer.

### Required contexts before and after

`Protect main` currently requires the six contexts listed above. After remote
activation its exact PR list is:

```text
dependency-preflight
policy-pack
native-tests
governance-static
flatpak-tests
flatpak
```

Only `ruleset-governance` is replaced. Publish eligibility requires those six
successful contexts for the exact release commit plus the seventh,
`governance-live`, with the provenance and decisive-step checks above.

### Local Task 8B implementation status

The local candidate implements the selected layout without changing remote
state. `governance-static` is a required, tokenless job with exact event-SHA
checkout, identity assertion and offline release check ordering.
`governance-live` has the exact protected-main producer condition, main-only
environment, checkout and identity assertions, and a single credentialed
decisive step. Structural checks reject conditional or error-tolerant actions,
dependency skips, extra mutating steps, secret exposure, producer drift and
command/order substitutions.

Publish preflight now uses `tools.release_evidence_fetch` to retrieve all
check-run and job pages from policy-derived same-origin API URLs. Bounded
responses, pagination loops/escapes, incomplete or changing totals, duplicate
or non-integer IDs, foreign details URLs and malformed payloads fail closed.
`tools.release_check_runs` then validates the complete stored evidence again,
including newest-check selection and the exact run/job/decisive-step producer.
The live governance checker also validates the exact environment branch policy,
repository-secret absence and environment-secret presence across all pages.

On PR #38, CodeQL high alert 185 traced the policy-configured `live_secret`
identifier through the repository-present and environment-missing diagnostics
to CLI stdout. That identifier is not the PAT value, and the path does not read
a credential value. The local follow-up still removes the unnecessary copy:
diagnostics now state only repository or environment scope while preserving
both failures. A synthetic configured-identifier regression covers direct
helper errors and actual CLI output. Alert status awaits the remote rerun.

`POLICY-RIT-GEN-038` remains open. This local implementation has not created
the environment or moved the secret or required context. The candidate PR is
expected to remain blocked by the old required context until the recorded
activation sequence below is executed.

Final local evidence passed 328 policy/tooling tests with one intentional
live-token skip, 42 focused policy unit tests, the strict policy-pack gate,
the app strict gate with 465 library tests plus stress/UI checks, and 84.6%
line coverage. Independent review found no remaining material issue after
null API payloads and credential-bearing transport errors were made explicitly
fail-closed. Logs use the `task8b-final-*` prefix under
`/tmp/riteed-p3-validation-hGkt8u/`.

The candidate PR intentionally emits no fake old context, so it remains blocked
by the current ruleset until the approved context transition. Before applying
that transition, capture a fresh complete `Protect main` response and
derive three local payloads: untouched before, after with exactly one
`ruleset-governance -> governance-static` replacement, and inverse with exactly
the reverse replacement. Preserve every other writable field, rule parameter,
bypass actor and condition. Assert the old/new context occurs exactly once in
the appropriate payload and diff the JSON before any PUT.

The activation order is: verify the candidate PR's six new checks; account for
the 15 unpublished integrated commits; capture and review exact ruleset
payloads; create/protect the environment; owner-enter and verify the environment
secret; have the owner test the credential's required read permissions outside
chat and capture admin metadata proving environment protection plus the secret
name; delete the repository copy; obtain separate approval for the one-field
ruleset PUT; and merge normally. The new environment job cannot execute trusted
new code before that merge. Afterward, require the resulting main SHA's six
static checks and seventh live main-push check, including repository-secret
absence. The old main workflow may fail live governance between secret deletion
and the authorized merge; that bounded bootstrap gap must be scheduled,
observed and not disguised as success.

Before the workflow merge, the captured inverse required-context PUT can
restore the old layout because the old `ruleset-governance` job still exists;
if its repository secret was already deleted, operational recovery additionally
requires explicit owner credential re-entry. After the merge, that job no
longer exists, so applying the inverse would intentionally block PRs rather
than restore service. Treat it only as fail-closed containment while preparing
a separately reviewed code/remote recovery, and prefer a reviewed roll-forward
under the new static contexts. Every such action remains owner-approval-gated.
No inverse may disable `Protect main`, touch `Protect version tags`, weaken
signatures/PR/deletion/non-fast-forward rules, alter signing/rollback
environments, create a fake old-context alias, or delete the protected
environment secret. There is no automatic secret rollback.

### Evidence still required for closure

Local Task 8B fixtures cover every acceptance row plus structural negatives
for conditions, fallbacks, credentials and producer binding. Payload fixtures
also cover missing/disabled rulesets, an unreviewed bypass actor and a missing
rollback reviewer. Under the recorded approval, record fresh environment,
secret-name and exact before/after/inverse
ruleset evidence; a representative ordinary PR; a fork or Dependabot PR; and
the exact merged-main push's static and live terminal results. Publish
eligibility must be tested against the same SHA. Until all of that exists,
`RIT-GEN-038` remains open.

## Before Activation, 2026-05-26

Repository: `cadric/riteed`

Remote `main`: `756598397804ec014129053517bd17cae417e803`

Local worktree `main`: `09fd1cdd0fb85e9d3b80caa0891715afc6d5afe1`

Classic branch protection:

```text
gh api repos/cadric/riteed/branches/main/protection
HTTP 404: Branch not protected
```

Ruleset `Protect main`:

```json
{
  "id": 16713108,
  "name": "Protect main",
  "target": "branch",
  "enforcement": "disabled",
  "conditions": { "ref_name": { "include": ["refs/heads/main"], "exclude": [] } },
  "bypass_actors": [{ "actor_id": 964797, "actor_type": "User", "bypass_mode": "always" }],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "required_status_checks",
      "parameters": {
        "do_not_enforce_on_create": true,
        "strict_required_status_checks_policy": true,
        "required_status_checks": [
          { "context": "policy-pack" },
          { "context": "native-tests" },
          { "context": "flatpak" },
          { "context": "flatpak-tests" }
        ]
      }
    }
  ]
}
```

Ruleset `Protect version tags`:

```json
{
  "id": 16713116,
  "name": "Protect version tags",
  "target": "tag",
  "enforcement": "disabled",
  "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
  "bypass_actors": [{ "actor_id": 964797, "actor_type": "User", "bypass_mode": "always" }],
  "rules": [
    { "type": "update" },
    { "type": "deletion" }
  ]
}
```

Latest `Validate` runs on remote `main`:

```text
2026-05-26T03:59:14Z schedule 756598397804... failure
2026-05-25T12:00:22Z push     756598397804... success
```

Current check-run context evidence for `commits/main` includes successful
`policy-pack`, `native-tests`, `flatpak`, and `flatpak-tests` contexts. The
scheduled `stress` job failed, but `stress` is intentionally not listed in the
existing `Protect main` required status checks.

## Approval Gate

Repository-owner approval was received in this Codex thread as
`approve remote rulesets` before the API writes were run.

## Apply Procedure Used

Capture before-state artifacts:

```bash
mkdir -p .agent/github-rulesets/2026-05-26
gh api repos/cadric/riteed/rulesets/16713108 \
  > .agent/github-rulesets/2026-05-26/protect-main.before.json
gh api repos/cadric/riteed/rulesets/16713116 \
  > .agent/github-rulesets/2026-05-26/protect-version-tags.before.json
gh api 'repos/cadric/riteed/commits/main/check-runs?per_page=100' \
  > .agent/github-rulesets/2026-05-26/main-check-runs.before.json
```

Prepare active payloads from the captured state:

```bash
jq '.enforcement = "active" |
    {name, target, enforcement, bypass_actors, conditions, rules}' \
  .agent/github-rulesets/2026-05-26/protect-main.before.json \
  > .agent/github-rulesets/2026-05-26/protect-main.active.json

jq '.enforcement = "active" |
    {name, target, enforcement, bypass_actors, conditions, rules}' \
  .agent/github-rulesets/2026-05-26/protect-version-tags.before.json \
  > .agent/github-rulesets/2026-05-26/protect-version-tags.active.json
```

Apply with the GitHub repository ruleset API:

```bash
gh api --method PUT repos/cadric/riteed/rulesets/16713108 \
  --input .agent/github-rulesets/2026-05-26/protect-main.active.json

gh api --method PUT repos/cadric/riteed/rulesets/16713116 \
  --input .agent/github-rulesets/2026-05-26/protect-version-tags.active.json
```

Record after-state artifacts:

```bash
gh api repos/cadric/riteed/rulesets/16713108 \
  > .agent/github-rulesets/2026-05-26/protect-main.after.json
gh api repos/cadric/riteed/rulesets/16713116 \
  > .agent/github-rulesets/2026-05-26/protect-version-tags.after.json
```

Closure requires both after-state files to show `"enforcement": "active"` and
the post-review governance fields listed below. After closure, `RIT-AUD-017`
was removed from `policy/release.policy.json` `planned_remediation`. Live
ruleset verification now belongs only to the isolated `ruleset-governance`
validation job; offline `policy_check --root app --strict` verifies static
workflow wiring and remains usable without GitHub credentials.

The reviewed bypass identity is exact-match only:
`Protect main` may list `User:964797:pull_request` (`@cadric`) for reviewed
break-glass pull requests. The release validator rejects `always`, typos,
missing reviewed bypass actors, additional live branch bypass actor tuples, and
any tag-ruleset bypass actor not listed in `policy/release.policy.json`.

The emergency rollback signing path uses the separate
`flatpak-beta-rollback` GitHub environment. Its required reviewer identity is
exact-match only: `User:964797` (`@cadric`). The live governance job verifies
that environment through the GitHub environments API and rejects missing or
additional required reviewer identities not listed in
`policy/release.policy.json`. `prevent_self_review` is intentionally `false`
for the rollback environment because Riteed is currently maintained by a single
repository owner; this downgrades the control to a deployment audit trail rather
than pretending independent review exists.

## After Activation, 2026-05-26

Fresh API reads after the PUT calls returned:

```text
Protect main branch active
Protect version tags tag active
```

The after-state artifacts are local continuity evidence under
`.agent/github-rulesets/2026-05-26/`.

## Post-review Status Check Update, 2026-05-26

After adding the isolated `ruleset-governance` validation job, `Protect main`
was updated through the same repository ruleset API to include
`ruleset-governance` in `required_status_checks`. Fresh release validation
then passed against the live ruleset payload.

## Post-review Governance Tightening, 2026-05-27

The reviewed policy now requires `Protect main` to include `pull_request`,
`required_signatures`, `required_status_checks`, and `non_fast_forward` rules,
with `strict_required_status_checks_policy: true` and an exact required-check
set matching
`signed_flatpak_publish.hard_requirements.required_validate_check_contexts`.
The `Protect version tags` ruleset must be active and have no bypass actors.
The `flatpak-beta-rollback` environment was provisioned on 2026-05-27 with
`User:964797` as the required reviewer, `wait_timer=0`, and
`prevent_self_review=false`.

The CI `ruleset-governance` job must use the `RULESET_GOVERNANCE_TOKEN`
repository secret rather than the ambient `github.token`. The expected
fine-grained PAT permissions are `Administration: Read-only`,
`Environments: Read-only`, and the default `Metadata: Read-only`. If GitHub
rejects the token, inspect the `x-accepted-github-permissions` response header
with `gh api -i` and add only the exact missing permission documented by the
API response.

`RIT-AUD-017` remains closed only when:

1. `Protect main` is updated through the ruleset API to use
   `User:964797:pull_request`, require signed commits, enforce strict required
   status checks, and match the policy check-run list exactly.
2. `Protect version tags` is updated through the ruleset API to have an empty
   `bypass_actors` list.
3. `GITHUB_TOKEN="$(gh auth token)" python3 -m tools.ruleset_governance_check`
   passes with a GitHub token that can read repository rulesets and
   environments.

## Solo-maintainer Pull-request Posture, 2026-05-28

Routine PR merges no longer require a self-impossible independent approval.
`Protect main` still requires a pull request, exact strict status checks,
signed commits, non-fast-forward protection, deletion protection, reviewed
thread resolution, and the reviewed `User:964797:pull_request` bypass actor for
emergencies. The only review change is:

```text
required_approving_review_count: 1 -> 0
require_last_push_approval: true -> false
```

This is the reviewed solo-maintainer model: PR + CI + signed commits + thread
resolution, without pretending that an independent reviewer exists. The
break-glass bypass remains `pull_request`-only, so emergency merges still leave
a PR audit trail rather than enabling direct `always` bypass.

Evidence recorded for the live ruleset update:

```text
a5e1e53a71bb679d45b0b8099965ac0f87df53e1cb435ee2cd1c0fed02cd18a1  docs/evidence/protect-main-solo-before-20260528.json
4bcf09556adfbb808233526b403f909cac8edda8ed52daae27318081467317d3  docs/evidence/protect-main-solo-after-20260528.json
```

The after-state was rechecked with:

```bash
GITHUB_TOKEN="$(gh auth token)" python3 -m tools.ruleset_governance_check
```

PR #12 then merged normally without `--admin`, after its head commit was
rewritten through GitHub's `createCommitOnBranch` API so the required-signatures
rule could verify the commit.

## Historical 2026-05-26 rollback command — not the P3 procedure

The following commands document the original 2026-05 activation rollback only.
Do not use them for RIT-GEN-038: they disable both rulesets, which the P3
procedure above explicitly forbids.

```bash
jq '.enforcement = "disabled" |
    {name, target, enforcement, bypass_actors, conditions, rules}' \
  .agent/github-rulesets/2026-05-26/protect-main.after.json \
  > .agent/github-rulesets/2026-05-26/protect-main.rollback.json

jq '.enforcement = "disabled" |
    {name, target, enforcement, bypass_actors, conditions, rules}' \
  .agent/github-rulesets/2026-05-26/protect-version-tags.after.json \
  > .agent/github-rulesets/2026-05-26/protect-version-tags.rollback.json

gh api --method PUT repos/cadric/riteed/rulesets/16713108 \
  --input .agent/github-rulesets/2026-05-26/protect-main.rollback.json
gh api --method PUT repos/cadric/riteed/rulesets/16713116 \
  --input .agent/github-rulesets/2026-05-26/protect-version-tags.rollback.json
```

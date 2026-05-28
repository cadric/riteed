# GitHub Ruleset Governance

This file records the `RIT-AUD-017` GitHub ruleset remediation: preflight
state, repository-owner approval boundary, applied API writes, after-state
evidence, and rollback commands.

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

## Rollback Command

If activation breaks required maintenance flow, restore the captured disabled
state with the same API endpoint:

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
